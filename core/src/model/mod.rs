//! Wire types for the v1 TCP protocol.
//!
//! Frames are length-prefixed (u32 LE) serde_json values; file blobs are sent
//! as a raw length-prefixed chunk following a `FileOffer`/`FileAccept`.

use serde::{Deserialize, Serialize};

pub const ALPN: &[&str] = &["nexdrop/1"];
/// Hard cap for text transfers, mirroring plan.md.
pub const MAX_TEXT: usize = 1_000_000;
/// Chunk size for file streaming and progress reporting granularity.
pub const CHUNK_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", content = "d")]
pub enum Frame {
    /// First frame on every connection, identifies the peer (id = cert fp).
    Hello { id: String, name: String },
    /// Initiated by the "add device" flow; `code` is the 6-digit code the
    /// initiator generated and shows on its own screen.
    PairRequest { id: String, name: String, code: String },
    PairAccept { name: String },
    PairReject,
    /// Receiver is mid-transfer; the sender must surface this as a failure.
    Busy,
    Text { text: String },
    FileOffer { id: String, files: Vec<FileEntry>, total: u64, code: String },
    FileAccept,
    FileReject,
    /// Transfer finished successfully (sender -> receiver).
    Done,
    Error { msg: String },
    /// Either side aborts an in-flight transfer.
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
}

/// Read one length-prefixed JSON frame. Returns `None` on clean EOF.
pub async fn read_frame(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
) -> anyhow::Result<Option<Frame>> {
    use tokio::io::AsyncReadExt;

    let mut len_buf = [0u8; 4];
    if stream.read_exact(&mut len_buf).await.is_err() {
        return Ok(None); // EOF before a full length prefix = clean close
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    anyhow::ensure!(len <= 64 * 1024 * 1024, "frame too large: {len}");
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(Some(serde_json::from_slice(&buf)?))
}

/// Write one length-prefixed JSON frame.
pub async fn write_frame(
    stream: &mut (impl tokio::io::AsyncWrite + Unpin),
    frame: &Frame,
) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;

    let buf = serde_json::to_vec(frame)?;
    stream.write_all(&(buf.len() as u32).to_le_bytes()).await?;
    stream.write_all(&buf).await?;
    stream.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn frames_roundtrip() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        write_frame(&mut a, &Frame::Hello { id: "x".into(), name: "n".into() })
            .await
            .unwrap();
        assert!(matches!(read_frame(&mut b).await.unwrap(), Some(Frame::Hello { .. })));
        write_frame(&mut a, &Frame::FileOffer {
            id: "x".into(),
            files: vec![FileEntry { name: "a.txt".into(), size: 3 }],
            total: 3,
            code: "123456".into(),
        })
        .await
        .unwrap();
        assert!(matches!(read_frame(&mut b).await.unwrap(), Some(Frame::FileOffer { .. })));
        drop(a); // close the writer side so the reader sees EOF
        assert!(read_frame(&mut b).await.unwrap().is_none());
    }
}
