//! Shared runtime state bridging the listener, mDNS, and FRB API calls.

use crate::crypto::Identity;
use crate::settings::Store;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, oneshot};

/// A device currently visible via mDNS. Unpaired devices show up here too —
/// the sidebar renders them as "online, not paired".
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub id: String,
    pub name: String,
    pub addr: std::net::IpAddr,
    pub port: u16,
}

/// Events pushed to the Dart side over the FRB stream.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CoreEvent {
    /// Device list changed (re-query paired + discovered from the API).
    DevicesChanged,
    /// Someone wants to pair with us; UI must answer via `decide_pair`.
    PairRequested { request_id: u64, name: String, code: String },
    PairCompleted { device_id: String, accepted: bool },
    /// A transfer began in either direction. The UI shows the progress ring
    /// over the corresponding box; `code` is the pairing code shown in it.
    TransferStart { outgoing: bool, is_text: bool, device_name: String, code: String, total: u64 },
    TransferProgress { outgoing: bool, is_text: bool, bytes: u64, total: u64 },
    TransferDone { outgoing: bool, is_text: bool },
    TransferFailed { outgoing: bool, is_text: bool, reason: String },
    /// Received text; UI keeps it until the matching TransferDone(kind=text).
    IncomingText { device_id: String, text: String },
}

pub struct CoreState {
    pub store: Store,
    pub identity: Identity,
    /// Devices currently online per mDNS, keyed by device id.
    pub devices: Mutex<HashMap<String, DiscoveredDevice>>,
    pub pending_pair: Mutex<HashMap<u64, oneshot::Sender<bool>>>,
    pub pending_file: Mutex<HashMap<u64, oneshot::Sender<bool>>>,
    /// Global conflict guard (a.md: while busy, any other transfer fails).
    pub busy: AtomicBool,
    /// Set by `cancel_transfer` from the UI; checked by transfer loops.
    pub cancelled: AtomicBool,
    pub listen_port: AtomicU16,
    pub next_request_id: AtomicU64,
    pub events: broadcast::Sender<CoreEvent>,
}

impl CoreState {
    pub fn emit(&self, event: CoreEvent) {
        let _ = self.events.send(event);
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn set_busy(&self, on: bool) {
        self.busy.store(on, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn set_cancelled(&self, on: bool) {
        self.cancelled.store(on, Ordering::SeqCst);
    }

    pub fn next_request_id(&self) -> u64 {
        self.next_request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Best-effort address for a device: live mDNS info first, then the
    /// address remembered at pair time.
    pub fn device_addr(&self, id: &str) -> Option<std::net::SocketAddr> {
        if let Some(d) = self.devices.lock().unwrap().get(id) {
            return Some(std::net::SocketAddr::new(d.addr, d.port));
        }
        self.store
            .find_paired(id)
            .and_then(|d| d.last_addr.and_then(|a| a.parse().ok()))
    }

    pub fn device_display_name(&self, id: &str) -> String {
        if let Some(d) = self.devices.lock().unwrap().get(id) {
            return d.name.clone();
        }
        self.store
            .find_paired(id)
            .map(|d| if d.alias.is_empty() { d.name } else { d.alias })
            .unwrap_or_else(|| id[..8.min(id.len())].to_string())
    }
}

/// Global singleton handle created by `api::start`.
pub static CORE: std::sync::OnceLock<Arc<CoreState>> = std::sync::OnceLock::new();

pub fn core() -> anyhow::Result<Arc<CoreState>> {
    CORE.get().cloned().ok_or_else(|| anyhow::anyhow!("core not started; call start() first"))
}

pub fn register_core(state: Arc<CoreState>) -> anyhow::Result<Arc<CoreState>> {
    CORE.set(state.clone())
        .map_err(|_| anyhow::anyhow!("core already started"))?;
    Ok(state)
}
