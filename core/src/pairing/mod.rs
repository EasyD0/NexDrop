//! Outgoing pairing: generate a code, ask the peer, persist on acceptance.

use crate::model::{self, Frame};
use crate::settings::{new_pairing_code, PairedDevice};
use crate::state::{CoreEvent, CoreState};
use crate::transfer::dial;
use anyhow::{anyhow, bail};
use std::sync::Arc;

/// Initiate pairing with a discovered device. Returns the 6-digit code that
/// the UI shows while waiting for the peer's decision.
pub async fn start_pairing(state: Arc<CoreState>, device_id: String) -> anyhow::Result<String> {
    let addr = state
        .device_addr(&device_id)
        .ok_or_else(|| anyhow!("设备不在线，请重新扫描"))?;
    let code = new_pairing_code();

    let result = async {
        let (mut io, peer_id, _, peer_fp) = dial(&state, addr).await?;
        model::write_frame(&mut io, &Frame::PairRequest {
            id: state.identity.fingerprint.clone(),
            name: state.store.settings().device_name,
            code: code.clone(),
        })
        .await?;
        match crate::model::read_frame(&mut io).await? {
            Some(Frame::PairAccept { name }) => {
                state.store.upsert_paired(PairedDevice {
                    id: peer_id.clone(),
                    name: name.clone(),
                    alias: String::new(),
                    fingerprint: peer_fp,
                    code: code.clone(),
                    last_addr: Some(addr.to_string()),
                })?;
                Ok::<(), anyhow::Error>(())
            }
            Some(Frame::PairReject) => bail!("对方拒绝了配对请求"),
            _ => bail!("对端异常响应"),
        }
    }
    .await;
    match &result {
        Ok(()) => state.emit(CoreEvent::PairCompleted { device_id, accepted: true }),
        Err(e) => {
            tracing::warn!("pairing failed: {e:#}");
            state.emit(CoreEvent::PairCompleted { device_id, accepted: false });
        }
    }
    result.map(|_| code)
}
