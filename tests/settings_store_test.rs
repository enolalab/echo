use auto_subtitle::settings::secrets::{MemoryStore, SecretsBackend};
use auto_subtitle::settings::*;

fn open(dir: &std::path::Path) -> SettingsStore {
    SettingsStore::open(
        dir,
        Box::new(MemoryStore::default()),
        SecretsBackend::Keyring,
    )
    .unwrap()
}

fn deepseek() -> ProviderProfile {
    ProviderProfile {
        id: String::new(),
        name: "DeepSeek".into(),
        kind: ProviderKind::DeepSeek,
        base_url: None,
        model: "deepseek-chat".into(),
    }
}

#[test]
fn missing_file_creates_defaults_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    assert_eq!(s.settings().profiles[0].name, "Ollama local");
    assert!(dir.path().join("settings.json").exists());
    assert_eq!(s.take_load_warning(), None);
}

#[test]
fn corrupt_file_is_backed_up_with_warning() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), "{not json").unwrap();
    let mut s = open(dir.path());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("settings.json.bak")).unwrap(),
        "{not json"
    );
    let w = s.take_load_warning().unwrap();
    assert!(w.contains("settings.json.bak"));
    assert_eq!(s.take_load_warning(), None);
    assert_eq!(s.settings().profiles.len(), 1);
}

#[test]
fn upsert_assigns_id_stores_key_outside_json_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let saved = s
        .upsert_profile(deepseek(), Some(" sk-123 ".into()))
        .unwrap();
    assert!(!saved.id.is_empty());
    assert_eq!(s.profile_key(&saved.id).unwrap().as_deref(), Some("sk-123"));
    assert!(s.has_profile_key(&saved.id));

    let json = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
    assert!(json.contains("DeepSeek") && !json.contains("sk-123"));
    assert!(!dir.path().join("settings.json.tmp").exists());

    // None keeps the key, Some("") deletes it
    let mut edited = saved.clone();
    edited.model = "deepseek-reasoner".into();
    s.upsert_profile(edited, None).unwrap();
    assert!(s.has_profile_key(&saved.id));
    s.upsert_profile(saved.clone(), Some(String::new()))
        .unwrap();
    assert!(!s.has_profile_key(&saved.id));

    let reopened = open(dir.path());
    assert_eq!(reopened.settings().profiles.len(), 2);
}

#[test]
fn upsert_rejects_invalid_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let mut dup = deepseek();
    dup.name = "ollama LOCAL".into();
    let err = s.upsert_profile(dup, None).unwrap_err();
    assert_eq!(err.to_string(), "Tên profile đã tồn tại");
}

#[test]
fn delete_moves_default_and_removes_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let ollama_id = s.settings().profiles[0].id.clone();
    let ds = s.upsert_profile(deepseek(), Some("k".into())).unwrap();
    s.set_default_profile(&ds.id).unwrap();
    s.delete_profile(&ds.id).unwrap();
    assert_eq!(
        s.settings().default_profile_id.as_deref(),
        Some(ollama_id.as_str())
    );
    assert_eq!(s.profile_key(&ds.id).unwrap(), None);
    s.delete_profile(&ollama_id).unwrap();
    assert_eq!(s.settings().default_profile_id, None);
    assert!(s.set_default_profile("nope").is_err());
}

#[test]
fn first_profile_becomes_default_when_none() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let id = s.settings().profiles[0].id.clone();
    s.delete_profile(&id).unwrap();
    let ds = s.upsert_profile(deepseek(), None).unwrap();
    assert_eq!(
        s.settings().default_profile_id.as_deref(),
        Some(ds.id.as_str())
    );
}

#[test]
fn general_and_stt_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let g = GeneralSettings {
        target_lang: "en".into(),
        ..Default::default()
    };
    s.set_general(g.clone()).unwrap();
    assert_eq!(open(dir.path()).settings().general, g);

    s.set_stt_key("w-1").unwrap();
    assert_eq!(s.stt_key().unwrap().as_deref(), Some("w-1"));
    s.set_stt_key("").unwrap();
    assert_eq!(s.stt_key().unwrap(), None);
}

#[test]
fn resolve_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let ds = s.upsert_profile(deepseek(), None).unwrap();
    assert_eq!(
        s.resolve_profile(None).unwrap().unwrap().name,
        "Ollama local"
    );
    assert_eq!(
        s.resolve_profile(Some("deepseek")).unwrap().unwrap().id,
        ds.id
    );
    assert_eq!(
        s.resolve_profile(Some(&ds.id)).unwrap().unwrap().name,
        "DeepSeek"
    );
    let err = s.resolve_profile(Some("claude")).unwrap_err().to_string();
    assert!(err.contains("Không tìm thấy profile 'claude'"));
    assert!(err.contains("Ollama local") && err.contains("DeepSeek"));
}
