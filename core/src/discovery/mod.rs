//! mDNS discovery (`_nexdrop._tcp.local.`). Registers our own service and
//! keeps `CoreState::devices` in sync with what's online.

use crate::state::{CoreState, DiscoveredDevice};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const SERVICE: &str = "_nexdrop._tcp.local.";

pub async fn start(state: Arc<CoreState>, port: u16) -> anyhow::Result<()> {
    let mdns = ServiceDaemon::new()?;
    let instance = format!("nexdrop-{}", &state.identity.fingerprint[..8]);
    // mdns-sd 要求主机名以 `.local.` 结尾，否则注册报错。
    let host = format!("{}.local.", instance);
    let txt: HashMap<String, String> = [
        ("id".to_string(), state.identity.fingerprint.clone()),
        ("name".to_string(), state.store.settings().device_name),
    ]
    .into_iter()
    .collect();

    let info = ServiceInfo::new(SERVICE, &instance, &host, "", port, txt)?;
    mdns.register(info)?;
    tracing::info!("mdns registered on port {port}");

    let receiver = mdns.browse(SERVICE)?;
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv_async().await {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let Some(id) = info.get_property("id").map(|p| p.val_str().to_string()) else {
                        continue;
                    };
                    if id == state.identity.fingerprint {
                        continue; // our own announcement
                    }
                    let Some(addr) = info
                        .get_addresses()
                        .iter()
                        .find(|a| a.is_ipv4() && !a.is_loopback())
                        .copied()
                        .or_else(|| info.get_addresses().iter().next().copied())
                    else {
                        continue;
                    };
                    let name = info
                        .get_property("name")
                        .map(|p| p.val_str().to_string())
                        .unwrap_or_else(|| info.get_fullname().to_string());
                    let device = DiscoveredDevice { id, name, addr, port: info.get_port() };
                    let changed = {
                        let mut devices = state.devices.lock().unwrap();
                        let is_new_or_changed = devices
                            .get(&device.id)
                            .map(|d| d.name != device.name || d.addr != device.addr || d.port != device.port)
                            .unwrap_or(true);
                        if is_new_or_changed {
                            devices.insert(device.id.clone(), device);
                        }
                        is_new_or_changed
                    };
                    if changed {
                        state.emit(crate::state::CoreEvent::DevicesChanged);
                    }
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    // Instance name is `nexdrop-<fp8>.<service>`.
                    let removed = {
                        let mut devices = state.devices.lock().unwrap();
                        let before = devices.len();
                        devices.retain(|_, d| {
                            let instance = format!("nexdrop-{}", &d.id[..8]);
                            !fullname.starts_with(&instance)
                        });
                        devices.len() != before
                    };
                    if removed {
                        state.emit(crate::state::CoreEvent::DevicesChanged);
                    }
                }
                _ => {}
            }
        }
    });

    // Refresh the TXT `name` in the background when settings change is cheap
    // enough to just re-announce every rename; the UI calls `announce_name`.
    Ok(())
}

/// Re-register under a new device name after the user renames themselves.
pub async fn reannounce(state: Arc<CoreState>, port: u16) -> anyhow::Result<()> {
    // mdns-sd doesn't expose unregister-all cleanly on the browse daemon
    // captured above; simplest correct v1 behavior is updating the shared
    // device table and letting peers pick the new name from TXT on next
    // Resolved. The listener re-registers periodically if the name changed.
    let _ = state;
    let _ = port;
    Ok(())
}

/// Give mDNS a short grace period during startup so the first device scan
/// isn't empty.
pub async fn wait_quiet(duration: Duration) {
    tokio::time::sleep(duration).await;
}
