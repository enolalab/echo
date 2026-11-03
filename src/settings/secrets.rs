use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Context;
use serde::{Deserialize, Serialize};

pub const SERVICE: &str = "com.echo.subtitles";
pub const STT_ACCOUNT: &str = "stt:openai";
const PROBE_ACCOUNT: &str = "probe:echo";

pub fn profile_account(id: &str) -> String {
    format!("profile:{id}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretsBackend {
    Keyring,
    File,
}

pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> anyhow::Result<Option<String>>;
    fn set(&self, account: &str, value: &str) -> anyhow::Result<()>;
    fn delete(&self, account: &str) -> anyhow::Result<()>;
}

pub struct KeyringStore;

impl SecretStore for KeyringStore {
    fn get(&self, account: &str) -> anyhow::Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE, account)?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set(&self, account: &str, value: &str) -> anyhow::Result<()> {
        keyring::Entry::new(SERVICE, account)?.set_password(value)?;
        Ok(())
    }

    fn delete(&self, account: &str) -> anyhow::Result<()> {
        match keyring::Entry::new(SERVICE, account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

pub struct FileStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    fn read_map(&self) -> anyhow::Result<BTreeMap<String, String>> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("invalid secrets file {}", self.path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e).with_context(|| format!("cannot read {}", self.path.display())),
        }
    }

    fn write_map(&self, map: &BTreeMap<String, String>) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(map)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

impl SecretStore for FileStore {
    fn get(&self, account: &str) -> anyhow::Result<Option<String>> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read_map()?.get(account).cloned())
    }

    fn set(&self, account: &str, value: &str) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut map = self.read_map()?;
        map.insert(account.to_string(), value.to_string());
        self.write_map(&map)
    }

    fn delete(&self, account: &str) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut map = self.read_map()?;
        if map.remove(account).is_some() {
            self.write_map(&map)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryStore(Mutex<BTreeMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, account: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(account)
            .cloned())
    }
    fn set(&self, account: &str, value: &str) -> anyhow::Result<()> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(account.to_string(), value.to_string());
        Ok(())
    }
    fn delete(&self, account: &str) -> anyhow::Result<()> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(account);
        Ok(())
    }
}

fn probe(store: &dyn SecretStore) -> bool {
    store.set(PROBE_ACCOUNT, "ok").is_ok()
        && matches!(store.get(PROBE_ACCOUNT), Ok(Some(v)) if v == "ok")
        && store.delete(PROBE_ACCOUNT).is_ok()
}

pub fn select_secret_store(
    primary: Box<dyn SecretStore>,
    fallback_path: &Path,
) -> (Box<dyn SecretStore>, SecretsBackend) {
    if probe(primary.as_ref()) {
        (primary, SecretsBackend::Keyring)
    } else {
        (
            Box::new(FileStore::new(fallback_path.to_path_buf())),
            SecretsBackend::File,
        )
    }
}

pub fn open_secret_store(dir: &Path) -> (Box<dyn SecretStore>, SecretsBackend) {
    select_secret_store(Box::new(KeyringStore), &dir.join("secrets.json"))
}
