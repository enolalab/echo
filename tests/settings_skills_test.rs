use auto_subtitle::settings::secrets::{MemoryStore, SecretsBackend};
use auto_subtitle::settings::{Settings, SettingsStore};
use auto_subtitle::skills::{GlossaryEntry, Skill};

fn open(dir: &std::path::Path) -> SettingsStore {
    SettingsStore::open(dir, Box::new(MemoryStore::default()), SecretsBackend::Keyring).unwrap()
}

fn mine(name: &str) -> Skill {
    Skill {
        id: String::new(),
        name: name.into(),
        description: String::new(),
        system_prompt: "p".into(),
        glossary: vec![GlossaryEntry { source: "a".into(), target: "b".into() }],
    }
}

#[test]
fn defaults_and_old_files_have_no_skills() {
    let s = Settings::with_defaults();
    assert!(s.skills.is_empty() && s.default_skill_id.is_none());
    let old: Settings = serde_json::from_str(r#"{"version":1,"profiles":[]}"#).unwrap();
    assert!(old.skills.is_empty());
}

#[test]
fn skills_all_lists_presets_then_user() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    st.upsert_skill(mine("Của tôi")).unwrap();
    let all = st.skills_all();
    assert_eq!(all.len(), 7);
    assert_eq!(all[0].id, "builtin:math");
    assert_eq!(all[6].name, "Của tôi");
}

#[test]
fn upsert_validates_persists_and_rejects_builtin() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    let s = st.upsert_skill(mine("  ML  ")).unwrap();
    assert!(!s.id.is_empty() && s.name == "ML");
    assert_eq!(open(dir.path()).settings().skills.len(), 1);

    let err = st.upsert_skill(mine("toán học")).unwrap_err().to_string();
    assert_eq!(err, "Tên skill đã tồn tại");

    let mut preset = st.find_skill("builtin:math").unwrap();
    preset.system_prompt = "hack".into();
    assert_eq!(st.upsert_skill(preset).unwrap_err().to_string(), "Không thể sửa skill có sẵn");

    let mut edited = s.clone();
    edited.description = "new".into();
    st.upsert_skill(edited).unwrap();
    assert_eq!(st.settings().skills[0].description, "new");
}

#[test]
fn delete_and_default() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    let s = st.upsert_skill(mine("ML")).unwrap();
    st.set_default_skill(Some(&s.id)).unwrap();
    st.delete_skill(&s.id).unwrap();
    assert_eq!(st.settings().default_skill_id, None);
    assert_eq!(st.delete_skill("builtin:math").unwrap_err().to_string(), "Không thể xóa skill có sẵn");
    st.set_default_skill(Some("builtin:math")).unwrap();
    assert_eq!(st.settings().default_skill_id.as_deref(), Some("builtin:math"));
    assert_eq!(st.set_default_skill(Some("nope")).unwrap_err().to_string(), "Không tìm thấy skill");
    st.set_default_skill(None).unwrap();
    assert_eq!(st.settings().default_skill_id, None);
}

#[test]
fn duplicate_names() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    let a = st.duplicate_skill("builtin:math").unwrap();
    let b = st.duplicate_skill("builtin:math").unwrap();
    assert_eq!(a.name, "Toán học (bản sao)");
    assert_eq!(b.name, "Toán học (bản sao) (2)");
    assert!(!a.is_builtin());
    assert_eq!(a.glossary, st.find_skill("builtin:math").unwrap().glossary);
    assert!(st.duplicate_skill("nope").is_err());
}

#[test]
fn resolve_rules() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    assert_eq!(st.resolve_skill(None).unwrap(), None);
    let s = st.upsert_skill(mine("ML")).unwrap();
    st.set_default_skill(Some(&s.id)).unwrap();
    assert_eq!(st.resolve_skill(None).unwrap().unwrap().id, s.id);
    assert_eq!(st.resolve_skill(Some("none")).unwrap(), None);
    assert_eq!(st.resolve_skill(Some("builtin:ai-ml")).unwrap().unwrap().name, "AI / Machine Learning");
    assert_eq!(st.resolve_skill(Some("ml")).unwrap().unwrap().id, s.id);
    assert_eq!(
        st.resolve_skill(Some("xyz")).unwrap_err().to_string(),
        "Không tìm thấy skill 'xyz'"
    );
}

#[test]
fn missing_default_is_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"version":1,"profiles":[],"default_skill_id":"gone"}"#,
    )
    .unwrap();
    let st = open(dir.path());
    assert_eq!(st.resolve_skill(None).unwrap(), None);
}

#[test]
fn import_export() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    let (file_name, json) = st.export_skill_json("builtin:math").unwrap();
    assert_eq!(file_name, "Toán học.echo-skill.json");
    let imported = st.import_skill_json(&json).unwrap();
    assert_eq!(imported.name, "Toán học (2)");
    assert!(!imported.is_builtin());
    let again = st.import_skill_json(&json).unwrap();
    assert_eq!(again.name, "Toán học (3)");
    assert_eq!(
        st.import_skill_json(r#"{"format":"x","version":1}"#).unwrap_err().to_string(),
        "File không phải skill Echo"
    );
    assert!(st.export_skill_json("nope").is_err());
}
