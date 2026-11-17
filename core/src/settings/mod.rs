//! JSON persistence for user settings and paired devices, under the OS
//! app-data dir (`%APPDATA%/nexdrop` on Windows).

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub fn default_data_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "nexdrop", "NexDrop")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".nexdrop"))
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path, fallback: T) -> anyhow::Result<T> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(fallback),
        Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
    }
}

fn save_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// User-editable settings (settings page in the UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub device_name: String,
    pub download_path: String,
    /// Whether incoming *file* transfers ask for a pairing-code confirmation.
    pub require_pairing_confirm: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            device_name: default_device_name(),
            download_path: default_download_path().to_string_lossy().into_owned(),
            require_pairing_confirm: true,
        }
    }
}

pub fn default_device_name() -> String {
    hostname().unwrap_or_else(|| "NexDrop 设备".into())
}

fn default_download_path() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|d| d.download_dir().map(|p| p.to_path_buf()))
        .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn hostname() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|s| !s.is_empty())
}

/// A device we have completed pairing with. `alias` is the local-only display
/// name (renameable in the UI); `code` is the code agreed at pair time and is
/// shown in the progress ring afterwards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    pub alias: String,
    pub fingerprint: String,
    pub code: String,
    pub last_addr: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PairStore {
    devices: Vec<PairedDevice>,
}

/// Shared, lock-protected persistence bundle held by `CoreState`.
pub struct Store {
    dir: PathBuf,
    settings: Mutex<Settings>,
    pairs: Mutex<PairStore>,
}

impl Store {
    pub fn open(dir: PathBuf) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let settings = load_json(&dir.join("settings.json"), Settings::default())?;
        let pairs = load_json(&dir.join("pairs.json"), PairStore::default())?;
        Ok(Self {
            dir,
            settings: Mutex::new(settings),
            pairs: Mutex::new(pairs),
        })
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn set_settings(&self, s: Settings) -> anyhow::Result<()> {
        *self.settings.lock().unwrap() = s;
        self.save_settings()
    }

    pub fn save_settings(&self) -> anyhow::Result<()> {
        let s = self.settings.lock().unwrap().clone();
        save_json(&self.dir.join("settings.json"), &s)
    }

    pub fn paired(&self) -> Vec<PairedDevice> {
        self.pairs.lock().unwrap().devices.clone()
    }

    pub fn find_paired(&self, id: &str) -> Option<PairedDevice> {
        self.pairs.lock().unwrap().devices.iter().find(|d| d.id == id).cloned()
    }

    pub fn upsert_paired(&self, device: PairedDevice) -> anyhow::Result<()> {
        let mut store = self.pairs.lock().unwrap();
        match store.devices.iter_mut().find(|d| d.id == device.id) {
            Some(existing) => *existing = device,
            None => store.devices.push(device),
        }
        save_json(&self.dir.join("pairs.json"), &*store)
    }

    pub fn remove_paired(&self, id: &str) -> anyhow::Result<bool> {
        let mut store = self.pairs.lock().unwrap();
        let before = store.devices.len();
        store.devices.retain(|d| d.id != id);
        let removed = store.devices.len() != before;
        if removed {
            save_json(&self.dir.join("pairs.json"), &*store)?;
        }
        Ok(removed)
    }

    pub fn set_alias(&self, id: &str, alias: &str) -> anyhow::Result<()> {
        let mut store = self.pairs.lock().unwrap();
        let device = store
            .devices
            .iter_mut()
            .find(|d| d.id == id)
            .with_context(|| format!("unknown device {id}"))?;
        device.alias = alias.to_string();
        save_json(&self.dir.join("pairs.json"), &*store)
    }

    /// Fingerprint allow-list used by the TOFU check on connect.
    pub fn known_fingerprint(&self, id: &str) -> Option<String> {
        self.find_paired(id).map(|d| d.fingerprint)
    }
}

/// Generate a 6-digit pairing code (shown on both screens during pairing and
/// reused in the progress ring afterwards).
pub fn new_pairing_code() -> String {
    use rand::Rng;
    format!("{:06}", rand::rng().random_range(0..1_000_000))
}
