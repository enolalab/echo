use auto_subtitle::skills::io::*;
use auto_subtitle::skills::model::{GlossaryEntry, Skill};

fn sample() -> Skill {
    Skill {
        id: "abc".into(),
        name: "ML của tôi".into(),
        description: "d".into(),
        system_prompt: "p {target_lang}".into(),
        glossary: vec![GlossaryEntry {
            source: "relu".into(),
            target: "".into(),
        }],
    }
}

#[test]
fn export_shape_has_no_id() {
    let v: serde_json::Value = serde_json::from_str(&export_json(&sample())).unwrap();
    assert_eq!(v["format"], "echo-skill");
    assert_eq!(v["version"], 1);
    assert_eq!(v["skill"]["name"], "ML của tôi");
    assert!(v["skill"].get("id").is_none());
}

#[test]
fn roundtrip_minus_id() {
    let back = parse_import(&export_json(&sample())).unwrap();
    assert_eq!(
        back,
        Skill {
            id: String::new(),
            ..sample()
        }
    );
}

#[test]
fn import_errors() {
    assert_eq!(
        parse_import(r#"{"format":"other","version":1,"skill":{}}"#).unwrap_err(),
        "File không phải skill Echo"
    );
    assert_eq!(
        parse_import(r#"{"format":"echo-skill","version":2,"skill":{}}"#).unwrap_err(),
        "Phiên bản skill không hỗ trợ: 2"
    );
    assert!(parse_import("{oops")
        .unwrap_err()
        .starts_with("File skill không hợp lệ:"));
    assert!(
        parse_import(r#"{"format":"echo-skill","version":1,"skill":{"name":1}}"#)
            .unwrap_err()
            .starts_with("File skill không hợp lệ:")
    );
    let big = format!(
        r#"{{"format":"echo-skill","version":1,"pad":"{}"}}"#,
        "x".repeat(MAX_IMPORT_BYTES)
    );
    assert_eq!(
        parse_import(&big).unwrap_err(),
        "File skill quá lớn (tối đa 1 MB)"
    );
}

#[test]
fn import_trims_fields() {
    let s = parse_import(
        r#"{"format":"echo-skill","version":1,"skill":{"name":"  X ","system_prompt":" p ","glossary":[{"source":" a ","target":" b "}]}}"#,
    )
    .unwrap();
    assert_eq!(s.name, "X");
    assert_eq!(s.glossary[0].source, "a");
}

#[test]
fn file_names_are_safe() {
    assert_eq!(
        safe_file_name("AI / ML: v2?"),
        "AI _ ML_ v2_.echo-skill.json"
    );
    assert_eq!(safe_file_name("Toán học"), "Toán học.echo-skill.json");
}
