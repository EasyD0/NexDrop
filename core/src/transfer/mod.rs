//! v1 TCP transfer: one TLS control connection per peer; text rides the
//! control channel, file blobs stream sequentially after a `FileOffer` /
//! `FileAccept` handshake. A single global busy flag implements the a.md
//! conflict rule: while transferring, other senders fail immediately.

use crate::model::{self, Frame};
use crate::settings::PairedDevice;
use crate::state::{CoreEvent, CoreState};
use anyhow::{anyhow, bail, Context};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

/// Type-erased TLS stream so client/server sides share helpers.
pub trait BoxIo: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> BoxIo for T {}
pub type BoxedIo = Box<dyn BoxIo>;

const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Peer certificate fingerprint from a completed TLS handshake (dial side).
fn fp_from_client(tls: &tokio_rustls::client::TlsStream<TcpStream>) -> anyhow::Result<String> {
    let (_, conn) = tls.get_ref();
    fp_of(conn.peer_certificates())
}

/// Peer certificate fingerprint from a completed TLS handshake (listen side).
fn fp_from_server(tls: &tokio_rustls::server::TlsStream<TcpStream>) -> anyhow::Result<String> {
    let (_, conn) = tls.get_ref();
    fp_of(conn.peer_certificates())
}

fn fp_of(certs: Option<&[rustls::pki_types::CertificateDer<'_>]>) -> anyhow::Result<String> {
    let cert = certs.ok_or_else(|| anyhow!("peer sent no certificate"))?;
    Ok(crate::crypto::fingerprint(cert[0].as_ref()))
}

/// Dial a peer, exchange hellos, verify TOFU. Returns the boxed stream plus
/// the peer's id, display name and certificate fingerprint.
pub async fn dial(state: &CoreState, addr: SocketAddr) -> anyhow::Result<(BoxedIo, String, String, String)> {
    let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| anyhow!("连接超时"))?
        .context("connect")?;
    let config = state.identity.tls_client_config()?;
    let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
    let tls = tokio::time::timeout(
        CONNECT_TIMEOUT,
        connector.connect("nexdrop".try_into()?, tcp),
    )
    .await
    .map_err(|_| anyhow!("TLS 握手超时"))??;
    let fp = fp_from_client(&tls)?;
    let mut io: BoxedIo = Box::new(tls);

    model::write_frame(&mut io, &Frame::Hello {
        id: state.identity.fingerprint.clone(),
        name: state.store.settings().device_name,
    })
    .await?;
    let (peer_id, peer_name) = match model::read_frame(&mut io).await? {
        Some(Frame::Hello { id, name }) => (id, name),
        _ => bail!("对端未发送 Hello"),
    };
    if let Some(known) = state.store.known_fingerprint(&peer_id) {
        if known != fp {
            bail!("设备指纹不匹配（可能重装或被冒充），请删除后重新配对");
        }
    }
    Ok((io, peer_id, peer_name, fp))
}

/// Listen for incoming connections forever. Called once from `api::start`.
pub async fn listen(state: Arc<CoreState>, listener: TcpListener) {
    let acceptor = TlsAcceptor::from(std::sync::Arc::new(
        match state.identity.tls_server_config() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("tls server config: {e:#}");
                return;
            }
        },
    ));
    let local = listener.local_addr().ok();
    if let Some(a) = local {
        state.listen_port.store(a.port(), std::sync::atomic::Ordering::SeqCst);
    }
    loop {
        match listener.accept().await {
            Ok((tcp, peer)) => {
                let state = state.clone();
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_incoming(state, acceptor, tcp, peer).await {
                        tracing::warn!("incoming from {peer}: {e:#}");
                    }
                });
            }
            Err(e) => {
                tracing::error!("accept: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }
    }
}

async fn handle_incoming(
    state: Arc<CoreState>,
    acceptor: TlsAcceptor,
    tcp: TcpStream,
    peer: SocketAddr,
) -> anyhow::Result<()> {
    let tls = acceptor.accept(tcp).await?;
    let fp = fp_from_server(&tls)?;
    let mut io: BoxedIo = Box::new(tls);

    let hello = match model::read_frame(&mut io).await? {
        Some(Frame::Hello { id, name }) => (id, name),
        _ => bail!("no hello"),
    };
    model::write_frame(&mut io, &Frame::Hello {
        id: state.identity.fingerprint.clone(),
        name: state.store.settings().device_name,
    })
    .await?;
    let (peer_id, _peer_name) = hello;

    let paired = state.store.find_paired(&peer_id);
    match &paired {
        Some(d) if d.fingerprint != fp => bail!("fingerprint mismatch for {peer_id}"),
        _ => {}
    }

    loop {
        let Some(frame) = model::read_frame(&mut io).await? else {
            return Ok(()); // peer closed
        };
        match frame {
            Frame::PairRequest { id, name, code } => {
                if id != peer_id {
                    bail!("pair request id mismatch");
                }
                let (tx, rx) = tokio::sync::oneshot::channel();
                let request_id = state.next_request_id();
                state.pending_pair.lock().unwrap().insert(request_id, tx);
                state.emit(CoreEvent::PairRequested { request_id, name: name.clone(), code: code.clone() });
                let accept = rx.await.unwrap_or(false);
                if accept {
                    state.store.upsert_paired(PairedDevice {
                        id: id.clone(),
                        name: name.clone(),
                        alias: String::new(),
                        fingerprint: fp.clone(),
                        code: code.clone(),
                        last_addr: Some(peer.to_string()),
                    })?;
                    model::write_frame(&mut io, &Frame::PairAccept {
                        name: state.store.settings().device_name,
                    })
                    .await?;
                } else {
                    model::write_frame(&mut io, &Frame::PairReject).await?;
                }
                state.emit(CoreEvent::PairCompleted { device_id: id, accepted: accept });
                if !accept {
                    return Ok(());
                }
            }
            _ if paired.is_none() => {
                // Unpaired peers may only pair.
                model::write_frame(&mut io, &Frame::Error { msg: "not paired".into() }).await?;
                return Ok(());
            }
            Frame::Text { text } => {
                if state.is_busy() {
                    model::write_frame(&mut io, &Frame::Busy).await?;
                    continue;
                }
                state.set_busy(true);
                state.set_cancelled(false);
                state.emit(CoreEvent::TransferStart {
                    outgoing: false, is_text: true,
                    device_name: display_name(&state, &peer_id),
                    code: paired.as_ref().map(|d| d.code.clone()).unwrap_or_default(),
                    total: text.len() as u64,
                });
                state.emit(CoreEvent::IncomingText { device_id: peer_id.clone(), text });
                model::write_frame(&mut io, &Frame::Done).await?;
                state.emit(CoreEvent::TransferDone { outgoing: false, is_text: true });
                state.set_busy(false);
            }
            Frame::FileOffer { files, total, code, .. } => {
                if state.is_busy() {
                    model::write_frame(&mut io, &Frame::Busy).await?;
                    continue;
                }
                state.set_busy(true);
                state.set_cancelled(false);
                let device_name = display_name(&state, &peer_id);
                state.emit(CoreEvent::TransferStart {
                    outgoing: false, is_text: false, device_name,
                    code: code.clone(), total,
                });
                let accepted = if state.store.settings().require_pairing_confirm {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    let request_id = state.next_request_id();
                    state.pending_file.lock().unwrap().insert(request_id, tx);
                    state.emit(CoreEvent::PairRequested { request_id, name: String::new(), code });
                    rx.await.unwrap_or(false)
                } else {
                    true
                };
                if !accepted {
                    model::write_frame(&mut io, &Frame::FileReject).await?;
                    state.emit(CoreEvent::TransferFailed {
                        outgoing: false, is_text: false, reason: "已拒绝".into(),
                    });
                    state.set_busy(false);
                    continue;
                }
                let dir = PathBuf::from(&state.store.settings().download_path);
                let result = receive_files(&mut io, &files, &dir, &state, false).await;
                match result {
                    Ok(()) => {
                        model::write_frame(&mut io, &Frame::Done).await?;
                        state.emit(CoreEvent::TransferDone { outgoing: false, is_text: false });
                    }
                    Err(e) => {
                        let _ = model::write_frame(&mut io, &Frame::Cancel).await;
                        state.emit(CoreEvent::TransferFailed {
                            outgoing: false, is_text: false, reason: format!("{e:#}"),
                        });
                    }
                }
                state.set_busy(false);
                return Ok(()); // one file transfer per connection
            }
            _ => {}
        }
    }
}

fn display_name(state: &CoreState, id: &str) -> String {
    state.device_display_name(id)
}

/// Stream `total` bytes per the offered entries to disk under `dir`,
/// emitting progress events. Shared by the incoming path.
async fn receive_files(
    io: &mut BoxedIo,
    files: &[model::FileEntry],
    dir: &Path,
    state: &Arc<CoreState>,
    outgoing: bool,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut done: u64 = 0;
    let total: u64 = files.iter().map(|f| f.size).sum();
    for file in files {
        if state.is_cancelled() {
            bail!("已取消");
        }
        let path = unique_path(dir, &sanitize(&file.name));
        let mut out = tokio::fs::File::create(&path).await?;
        let mut remaining = file.size;
        let mut buf = vec![0u8; model::CHUNK_SIZE.min(file.size.max(1) as usize)];
        while remaining > 0 {
            if state.is_cancelled() {
                let _ = out.sync_all().await;
                let _ = tokio::fs::remove_file(&path).await;
                bail!("已取消");
            }
            let want = (remaining.min(buf.len() as u64)) as usize;
            let n = io.read(&mut buf[..want]).await?;
            if n == 0 {
                bail!("对端提前断开");
            }
            out.write_all(&buf[..n]).await?;
            remaining -= n as u64;
            done += n as u64;
            state.emit(CoreEvent::TransferProgress {
                outgoing, is_text: false, bytes: done, total,
            });
        }
        out.sync_all().await?;
    }
    Ok(())
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let base = Path::new(&cleaned)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    if base.is_empty() { "file".into() } else { base }
}

/// Avoid overwriting: `a.txt` -> `a (1).txt`.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    if !path.exists() {
        return path;
    }
    let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = Path::new(name).extension().map(|s| s.to_string_lossy().into_owned());
    for i in 1..1000u32 {
        let candidate = match &ext {
            Some(e) => format!("{stem} ({i}).{e}"),
            None => format!("{stem} ({i})"),
        };
        let p = dir.join(candidate);
        if !p.exists() {
            return p;
        }
    }
    dir.join(format!("{stem}-{i_usize}", i_usize = std::process::id()))
}

// ---------------------------------------------------------------- sending

/// Send text over a fresh connection; resolves when the peer acknowledges.
pub async fn send_text(state: Arc<CoreState>, device_id: String, text: String) -> anyhow::Result<()> {
    if text.len() > model::MAX_TEXT {
        bail!("文本超过 1 MB 上限");
    }
    if state.is_busy() {
        bail!("已有传输正在进行");
    }
    let addr = state.device_addr(&device_id).ok_or_else(|| anyhow!("设备不在线"))?;
    let code = state.store.find_paired(&device_id).map(|d| d.code).unwrap_or_default();
    let device_name = state.device_display_name(&device_id);

    state.set_busy(true);
    state.set_cancelled(false);
    state.emit(CoreEvent::TransferStart {
        outgoing: true, is_text: true, device_name, code, total: text.len() as u64,
    });
    let result = async {
        let (mut io, _, _, _) = dial(&state, addr).await?;
        model::write_frame(&mut io, &Frame::Text { text }).await?;        match model::read_frame(&mut io).await? {
            Some(Frame::Done) => Ok(()),
            Some(Frame::Busy) => bail!("对方正在传输，请稍后再试"),
            Some(Frame::Error { msg }) => bail!("{msg}"),
            _ => bail!("对端异常响应"),
        }
    }
    .await;
    match &result {
        Ok(()) => state.emit(CoreEvent::TransferDone { outgoing: true, is_text: true }),
        Err(e) => state.emit(CoreEvent::TransferFailed { outgoing: true, is_text: true, reason: format!("{e:#}") }),
    }
    state.set_busy(false);
    result
}

pub async fn send_files(state: Arc<CoreState>, device_id: String, paths: Vec<String>) -> anyhow::Result<()> {
    if state.is_busy() {
        bail!("已有传输正在进行");
    }
    let mut files = Vec::new();
    let mut total: u64 = 0;
    for p in &paths {
        let meta = tokio::fs::metadata(p).await.with_context(|| format!("读取 {p}"))?;
        if !meta.is_file() {
            bail!("暂不支持目录：{p}");
        }
        total += meta.len();
        files.push(model::FileEntry {
            name: Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            size: meta.len(),
        });
    }
    let addr = state.device_addr(&device_id).ok_or_else(|| anyhow!("设备不在线"))?;
    let code = state.store.find_paired(&device_id).map(|d| d.code).unwrap_or_default();
    let device_name = state.device_display_name(&device_id);

    state.set_busy(true);
    state.set_cancelled(false);
    state.emit(CoreEvent::TransferStart {
        outgoing: true, is_text: false, device_name, code: code.clone(), total,
    });

    let result = async {
        let (mut io, peer_id, _, _) = dial(&state, addr).await?;
        if state.store.known_fingerprint(&peer_id).is_none() {
            bail!("设备尚未配对");
        }
        let mut readers = Vec::new();
        for p in &paths {
            readers.push(tokio::fs::File::open(p).await?);
        }
        model::write_frame(&mut io, &Frame::FileOffer {
            id: state.identity.fingerprint.clone(),
            files: files.clone(),
            total,
            code: code.clone(),
        })
        .await?; // code cloned; original moved below is fine

        match model::read_frame(&mut io).await? {
            Some(Frame::FileAccept) => {}
            Some(Frame::FileReject) => bail!("对方拒绝了接收"),
            Some(Frame::Busy) => bail!("对方正在传输，请稍后再试"),
            Some(Frame::Error { msg }) => bail!("{msg}"),
            _ => bail!("对端异常响应"),
        }
        let mut sent: u64 = 0;
        let mut buf = vec![0u8; model::CHUNK_SIZE];
        for mut file in readers {
            loop {
                if state.is_cancelled() {
                    bail!("已取消");
                }
                let n = file.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                io.write_all(&buf[..n]).await?;
                sent += n as u64;
                state.emit(CoreEvent::TransferProgress {
                    outgoing: true, is_text: false, bytes: sent, total,
                });
            }
        }
        io.flush().await?;
        match model::read_frame(&mut io).await? {
            Some(Frame::Done) => Ok(()),
            Some(Frame::Cancel) => bail!("对方取消了接收"),
            Some(Frame::Error { msg }) => bail!("{msg}"),
            _ => bail!("对端异常响应"),
        }
    }
    .await;
    match &result {
        Ok(()) => state.emit(CoreEvent::TransferDone { outgoing: true, is_text: false }),
        Err(e) => state.emit(CoreEvent::TransferFailed { outgoing: true, is_text: false, reason: format!("{e:#}") }),
    }
    state.set_busy(false);
    result
}

/// User pressed 中断/取消. Best-effort: the transfer loop polls the flag.
pub fn cancel_transfer(state: &Arc<CoreState>) {
    state.set_cancelled(true);
}
