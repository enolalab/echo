use auto_subtitle::settings::secrets::*;

struct Broken;
impl SecretStore for Broken {
    fn get(&self, _: &str) -> anyhow::Result<Option<String>> {
        anyhow::bail!("no keychain")
    }
    fn set(&self, _: &str, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("no keychain")
    }
    fn delete(&self, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("no keychain")
    }
}

#[test]
fn accounts() {
    assert_eq!(profile_account("abc"), "profile:abc");
    assert_eq!(STT_ACCOUNT, "stt:openai");
    assert_eq!(SERVICE, "com.echo.subtitles");
}

#[test]
fn file_store_roundtrip_and_delete() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");
    let s = FileStore::new(path.clone());
    assert_eq!(s.get("a").unwrap(), None);
    s.set("a", "1").unwrap();
    s.set("b", "2").unwrap();
    assert_eq!(s.get("a").unwrap().as_deref(), Some("1"));
    s.delete("a").unwrap();
    s.delete("missing").unwrap();
    assert_eq!(s.get("a").unwrap(), None);
    // persisted across instances
    assert_eq!(FileStore::new(path).get("b").unwrap().as_deref(), Some("2"));
}

#[cfg(unix)]
#[test]
fn file_store_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");
    FileStore::new(path.clone()).set("a", "1").unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn memory_store_roundtrip() {
    let s = MemoryStore::default();
    s.set("x", "y").unwrap();
    assert_eq!(s.get("x").unwrap().as_deref(), Some("y"));
    s.delete("x").unwrap();
    assert_eq!(s.get("x").unwrap(), None);
}

#[test]
fn select_falls_back_to_file_when_primary_broken() {
    let dir = tempfile::tempdir().unwrap();
    let (store, backend) = select_secret_store(Box::new(Broken), &dir.path().join("secrets.json"));
    assert_eq!(backend, SecretsBackend::File);
    store.set("a", "1").unwrap();
    assert!(dir.path().join("secrets.json").exists());
}

#[test]
fn select_keeps_working_primary_and_cleans_probe() {
    let dir = tempfile::tempdir().unwrap();
    let (store, backend) =
        select_secret_store(Box::new(MemoryStore::default()), &dir.path().join("secrets.json"));
    assert_eq!(backend, SecretsBackend::Keyring);
    assert_eq!(store.get("probe:echo").unwrap(), None);
    assert!(!dir.path().join("secrets.json").exists());
}

#[test]
fn backend_serializes_lowercase() {
    assert_eq!(serde_json::to_string(&SecretsBackend::File).unwrap(), "\"file\"");
    assert_eq!(serde_json::to_string(&SecretsBackend::Keyring).unwrap(), "\"keyring\"");
}
