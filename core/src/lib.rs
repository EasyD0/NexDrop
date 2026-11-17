//! NexDrop core: discovery, pairing, and encrypted transfer.
//!
//! The crate is built for both targets from the same source:
//! Android loads it as `libnexdrop_core.so`, Windows as `nexdrop_core.dll`.
//! Nothing in here may depend on a platform UI toolkit.

mod frb_generated; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */

pub mod api;
pub mod crypto;
pub mod discovery;
pub mod model;
pub mod pairing;
pub mod settings;
pub mod state;
pub mod transfer;

/// Core version, surfaced in the UI at startup.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_not_empty() {
        assert!(!super::version().is_empty());
    }
}
