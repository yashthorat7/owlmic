//! The one file Owlmic keeps: `%APPDATA%\Owlmic\owlmic.json` on Windows,
//! or `$XDG_CONFIG_HOME/owlmic/owlmic.json` on Linux.
//! Secrets go through a [`Protector`] (DPAPI on Windows) before they are written.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Encrypts secrets at rest. On Windows this is DPAPI for the current user.
pub trait Protector: Send + Sync {
    fn protect(&self, plain: &[u8]) -> Vec<u8>;
    fn unprotect(&self, sealed: &[u8]) -> Option<Vec<u8>>;
}

/// For tests and development builds: stores secrets as they are.
pub struct PlainProtector;

impl Protector for PlainProtector {
    fn protect(&self, plain: &[u8]) -> Vec<u8> {
        plain.to_vec()
    }
    fn unprotect(&self, sealed: &[u8]) -> Option<Vec<u8>> {
        Some(sealed.to_vec())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Versioned {
    pub value: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityRecord {
    /// Base64 of the protected 32-byte private scalar.
    pub private_protected: String,
    /// Base64 of the 65-byte public point.
    pub public: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneRecord {
    pub name: String,
    pub model: String,
    /// Base64 of the phone's 65-byte public key, so a reinstalled phone is treated as new.
    pub static_pub: String,
    #[serde(default)]
    pub blocked: bool,
    /// Seconds since 1970.
    #[serde(default)]
    pub last_seen: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerRecord {
    /// Set while Owlmic has the PC's speakers muted, so a crash can't leave them silent.
    #[serde(default)]
    pub muted_by_owlmic: Option<String>,
    /// Outputs where muting also silenced what the phone hears; never muted again.
    #[serde(default)]
    pub unmutable_devices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StoreData {
    #[serde(default)]
    pub identity: Option<IdentityRecord>,
    /// 32 hex characters.
    #[serde(default)]
    pub pc_id: Option<String>,
    /// Approved and blocked phones, keyed by phone id (hex).
    #[serde(default)]
    pub phones: BTreeMap<String, PhoneRecord>,
    /// Settings that belong to the PC, not to a pairing (`general.*`).
    #[serde(default)]
    pub general: BTreeMap<String, Versioned>,
    /// Each pairing's settings, keyed by phone id (hex).
    #[serde(default)]
    pub pairings: BTreeMap<String, BTreeMap<String, Versioned>>,
    #[serde(default)]
    pub speaker: SpeakerRecord,
}

/// Returns the platform-appropriate directory for Owlmic settings:
/// - Windows: `%APPDATA%\Owlmic`
/// - Linux: `$XDG_CONFIG_HOME/owlmic` (defaults to `~/.config/owlmic`)
pub fn default_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Owlmic"))
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            Some(PathBuf::from(xdg).join("owlmic"))
        } else if let Some(home) = std::env::var_os("HOME") {
            Some(PathBuf::from(home).join(".config").join("owlmic"))
        } else {
            None
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            Some(PathBuf::from(home).join(".config").join("owlmic"))
        } else {
            None
        }
    }
}

/// Returns the platform-appropriate default path for `owlmic.json`.
pub fn default_path() -> Option<PathBuf> {
    default_dir().map(|d| d.join("owlmic.json"))
}

pub struct Store {
    path: PathBuf,
    data: Mutex<StoreData>,
    protector: Box<dyn Protector>,
}

impl Store {
    /// Opens the default settings store for this platform.
    pub fn open_default(protector: Box<dyn Protector>) -> Option<Self> {
        default_path().map(|p| Self::open(p, protector))
    }

    /// Opens the store at `path`. A missing file starts empty; an unreadable one is kept as
    /// `owlmic.json.bad` and Owlmic starts fresh rather than not at all.
    pub fn open(path: impl Into<PathBuf>, protector: Box<dyn Protector>) -> Self {
        let path = path.into();
        let data = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                let _ = std::fs::rename(&path, path.with_extension("json.bad"));
                StoreData::default()
            }),
            Err(_) => StoreData::default(),
        };
        Self {
            path,
            data: Mutex::new(data),
            protector,
        }
    }

    pub fn read<R>(&self, f: impl FnOnce(&StoreData) -> R) -> R {
        f(&self.data.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// Changes the data and writes the file atomically: a temporary file, then a rename.
    pub fn update<R>(&self, f: impl FnOnce(&mut StoreData) -> R) -> R {
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        let r = f(&mut data);
        let _ = write_atomic(
            &self.path,
            &serde_json::to_vec_pretty(&*data).expect("store serializes"),
        );
        r
    }

    pub fn protect(&self, plain: &[u8]) -> Vec<u8> {
        self.protector.protect(plain)
    }

    pub fn unprotect(&self, sealed: &[u8]) -> Option<Vec<u8>> {
        self.protector.unprotect(sealed)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("owlmic-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("owlmic.json")
    }

    #[test]
    fn changes_survive_a_reopen() {
        let path = temp("reopen");
        let store = Store::open(&path, Box::new(PlainProtector));
        store.update(|d| d.pc_id = Some("ab".repeat(16)));
        let again = Store::open(&path, Box::new(PlainProtector));
        assert_eq!(again.read(|d| d.pc_id.clone()), Some("ab".repeat(16)));
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn a_damaged_file_is_set_aside_and_the_store_starts_fresh() {
        let path = temp("damaged");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{not json").unwrap();
        let store = Store::open(&path, Box::new(PlainProtector));
        assert_eq!(store.read(|d| d.clone()), StoreData::default());
        assert!(path.with_extension("json.bad").exists());
    }

    #[test]
    fn platform_default_path_ends_with_owlmic_json() {
        if let Some(path) = default_path() {
            assert!(path.ends_with("owlmic.json"));
        }
    }
}
