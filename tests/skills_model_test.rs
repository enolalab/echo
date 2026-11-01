use auto_subtitle::skills::model::*;

fn skill(id: &str, name: &str) -> Skill {
    Skill {
        id: id.into(),
        name: name.into(),
        description: String::new(),
        system_prompt: "Translate carefully.".into(),
        glossary: vec![],
    }
}

fn entry(s: &str, t: &str) -> GlossaryEntry {
    GlossaryEntry { source: s.into(), target: t.into() }
}

#[test]
fn builtin_detection_and_constants() {
    assert!(skill("builtin:math", "x").is_builtin());
    assert!(!skill("abc", "x").is_builtin());
    assert_eq!(NO_SKILL, "none");
    assert_eq!(BUILTIN_PREFIX, "builtin:");
}

#[test]
fn normalized_trims_everything() {
    let s = Skill {
        id: "1".into(),
        name: "  ML  ".into(),
        description: " d ".into(),
        system_prompt: "  p  ".into(),
        glossary: vec![entry(" relu ", " (x) ")],
    }
    .normalized();
    assert_eq!(s.name, "ML");
    assert_eq!(s.description, "d");
    assert_eq!(s.system_prompt, "p");
    assert_eq!(s.glossary, vec![entry("relu", "(x)")]);
}

#[test]
fn validation_rules() {
    let others = vec![skill("builtin:math", "Toán học"), skill("u1", "Của tôi")];
    assert!(skill("new", "ML").validate(&others).is_ok());
    assert_eq!(skill("new", " ").validate(&others).unwrap_err(), "Tên skill không được để trống");
    assert_eq!(skill("new", "toán HỌC").validate(&others).unwrap_err(), "Tên skill đã tồn tại");

    let mut s = skill("new", "ML");
    s.system_prompt = "  ".into();
    assert_eq!(s.validate(&others).unwrap_err(), "System prompt không được để trống");
    s.system_prompt = "a".repeat(MAX_PROMPT_CHARS + 1);
    assert_eq!(s.validate(&others).unwrap_err(), "System prompt tối đa 8000 ký tự");

    let mut s = skill("new", "ML");
    s.glossary = (0..=MAX_GLOSSARY).map(|i| entry(&format!("t{i}"), "")).collect();
    assert_eq!(s.validate(&others).unwrap_err(), "Glossary tối đa 1000 thuật ngữ");

    let mut s = skill("new", "ML");
    s.glossary = vec![entry(" ", "x")];
    assert_eq!(s.validate(&others).unwrap_err(), "Thuật ngữ gốc không được để trống");
    s.glossary = vec![entry("ReLU", "a"), entry("relu", "b")];
    assert_eq!(s.validate(&others).unwrap_err(), "Thuật ngữ bị trùng: relu");
}

#[test]
fn prompt_length_counts_chars_not_bytes() {
    let mut s = skill("new", "ML");
    s.system_prompt = "đ".repeat(MAX_PROMPT_CHARS);
    assert!(s.validate(&[]).is_ok());
}

#[test]
fn unique_name_appends_suffix() {
    let taken = vec!["A".to_string(), "a (2)".to_string()];
    assert_eq!(unique_name("B", &taken), "B");
    assert_eq!(unique_name("A", &taken), "A (3)");
}

#[test]
fn serde_defaults() {
    let s: Skill = serde_json::from_str(r#"{"id":"x","name":"n","system_prompt":"p"}"#).unwrap();
    assert!(s.glossary.is_empty() && s.description.is_empty());
    let e: GlossaryEntry = serde_json::from_str(r#"{"source":"softmax"}"#).unwrap();
    assert_eq!(e.target, "");
}
