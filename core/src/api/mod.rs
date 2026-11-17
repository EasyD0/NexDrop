//! Flutter-facing API. Everything the Dart side can call lives here; types
//! in signatures must be FRB-translatable (plain structs/enums of primitives).

use crate::settings::{PairedDevice, Settings};
use crate::state::{core, register_core, CoreEvent, CoreState, DiscoveredDevice};
use crate::transfer;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use std::sync::Arc;

pub fn version() -> String {
    crate::version().to_string()
}

/// Bind the listener, start mDNS, register global state. Idempotent-ish:
/// calling twice is an error.
pub async fn start() -> anyhow::Result<String> {
    let dir = crate::settings::default_data_dir();
    let store = crate::settings::Store::open(dir)?;
    let identity = crate::crypto::Identity::load_or_create(&crate::settings::default_data_dir())?;
    let (events, _) = tokio::sync::broadcast::channel(256);
    let state = Arc::new(CoreState {
        store,
        identity,
        devices: Default::default(),
        pending_pair: Default::default(),
        pending_file: Default::default(),
        busy: Default::default(),
        cancelled: Default::default(),
        listen_port: Default::default(),
        next_request_id: Default::default(),
        events,
    });
    register_core(state.clone())?;

    let listener = TcpListener::bind(("0.0.0.0", 0)).await?;
    let port = listener.local_addr()?.port();
    state.listen_port.store(port, std::sync::atomic::Ordering::SeqCst);
    tokio::spawn(crate::transfer::listen(state.clone(), listener));
    crate::discovery::start(state.clone(), port).await?;
    Ok(format!("listening on {port}"))
}

/// Push every core event to Dart. Call once at startup.
pub async fn subscribe_events(sink: StreamSink<CoreEvent>) -> anyhow::Result<()> {
    let state = core()?;
    let mut rx = state.events.subscribe();
    while let Ok(event) = rx.recv().await {
        sink.add(event).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    Ok(())
}

// Re-export StreamSink from generated bindings so codegen finds it.
use crate::frb_generated::StreamSink;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceView {
    pub id: String,
    /// Local alias if set, else the peer's own name.
    pub display_name: String,
    /// The peer's advertised name (for the alias editor's placeholder).
    pub device_name: String,
    pub online: bool,
    pub paired: bool,
}

/// Sidebar content: paired devices plus anything currently discovered.
pub fn list_devices() -> anyhow::Result<Vec<DeviceView>> {
    let state = core()?;
    let discovered: Vec<DiscoveredDevice> = state.devices.lock().unwrap().values().cloned().collect();
    let paired: Vec<PairedDevice> = state.store.paired();
    let mut out: Vec<DeviceView> = Vec::new();
    for d in &paired {
        out.push(DeviceView {
            id: d.id.clone(),
            display_name: if d.alias.is_empty() { d.name.clone() } else { d.alias.clone() },
            device_name: d.name.clone(),
            online: discovered.iter().any(|x| x.id == d.id),
            paired: true,
        });
    }
    for d in discovered {
        if !out.iter().any(|v| v.id == d.id) {
            out.push(DeviceView {
                id: d.id.clone(),
                display_name: d.name.clone(),
                device_name: d.name.clone(),
                online: true,
                paired: false,
            });
        }
    }
    Ok(out)
}

pub fn get_settings() -> anyhow::Result<Settings> {
    Ok(core()?.store.settings())
}

pub fn set_settings(settings: Settings) -> anyhow::Result<()> {
    let state = core()?;
    let old = state.store.settings();
    state.store.set_settings(settings.clone())?;
    if old.device_name != settings.device_name {
        // Re-announce under the new name.
        let port = state.listen_port.load(std::sync::atomic::Ordering::SeqCst);
        tokio::spawn(crate::discovery::reannounce(state.clone(), port));
    }
    Ok(())
}

/// Device list entry selected in the "add device" sheet -> pair with it.
/// Returns the generated pairing code.
pub async fn pair(device_id: String) -> anyhow::Result<String> {
    crate::pairing::start_pairing(core()?, device_id).await
}

pub fn decide_pair(request_id: u64, accept: bool) -> anyhow::Result<()> {
    let state = core()?;
    let tx = state.pending_pair.lock().unwrap().remove(&request_id);
    match tx {
        Some(tx) => tx.send(accept).ok().context_send(),
        None => bail!("配对请求已失效"),
    }
}

pub fn decide_file(request_id: u64, accept: bool) -> anyhow::Result<()> {
    let state = core()?;
    let tx = state.pending_file.lock().unwrap().remove(&request_id);
    match tx {
        Some(tx) => tx.send(accept).ok().context_send(),
        None => bail!("接收请求已失效"),
    }
}

pub fn rename_device(id: String, alias: String) -> anyhow::Result<()> {
    core()?.store.set_alias(&id, &alias)?;
    core()?.emit(CoreEvent::DevicesChanged);
    Ok(())
}

pub fn remove_device(id: String) -> anyhow::Result<()> {
    core()?.store.remove_paired(&id)?;
    core()?.emit(CoreEvent::DevicesChanged);
    Ok(())
}

pub async fn send_text(device_id: String, text: String) -> anyhow::Result<()> {
    transfer::send_text(core()?, device_id, text).await
}

pub async fn send_files(device_id: String, paths: Vec<String>) -> anyhow::Result<()> {
    transfer::send_files(core()?, device_id, paths).await
}

pub fn cancel_transfer() -> anyhow::Result<()> {
    transfer::cancel_transfer(&core()?);
    Ok(())
}

use anyhow::Context as _;

/// tiny helper keeping `tx.send(...)` fallible-free
trait ContextSend {
    fn context_send(self) -> anyhow::Result<()>;
}
impl ContextSend for Option<()> {
    fn context_send(self) -> anyhow::Result<()> {
        self.context("决策通道已关闭")
    }
}
