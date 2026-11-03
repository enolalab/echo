use auto_subtitle::settings::model::*;

fn profile(id: &str, name: &str, kind: ProviderKind, base: Option<&str>) -> ProviderProfile {
    ProviderProfile {
        id: id.into(),
        name: name.into(),
        kind,
        base_url: base.map(String::from),
        model: "m".into(),
    }
}

#[test]
fn kind_serializes_to_spec_values() {
    let pairs = [
        (ProviderKind::OpenAi, "\"openai\""),
        (ProviderKind::OpenAiCompatible, "\"openai_compatible\""),
        (ProviderKind::Anthropic, "\"anthropic\""),
        (
            ProviderKind::AnthropicCompatible,
            "\"anthropic_compatible\"",
        ),
        (ProviderKind::DeepSeek, "\"deepseek\""),
        (ProviderKind::Ollama, "\"ollama\""),
    ];
    for (k, json) in pairs {
        assert_eq!(serde_json::to_string(&k).unwrap(), json);
        assert_eq!(serde_json::from_str::<ProviderKind>(json).unwrap(), k);
    }
}

#[test]
fn defaults_have_one_ollama_profile_as_default() {
    let s = Settings::with_defaults();
    assert_eq!(s.version, SETTINGS_VERSION);
    assert_eq!(s.profiles.len(), 1);
    let p = &s.profiles[0];
    assert_eq!(p.name, "Ollama local");
    assert_eq!(p.kind, ProviderKind::Ollama);
    assert_eq!(p.model, "qwen2.5:7b");
    assert_eq!(s.default_profile_id.as_deref(), Some(p.id.as_str()));
    assert_eq!(s.general, GeneralSettings::default());
    assert_eq!(s.general.whisper_model, "base");
    assert_eq!(s.general.target_lang, "vi");
    assert!(s.general.bilingual);
    assert_eq!(s.general.style_preset, "youtube_yellow");
    assert!(!s.general.use_cloud_stt);
}

#[test]
fn effective_base_url_uses_kind_default_and_trims() {
    assert_eq!(
        profile("1", "a", ProviderKind::DeepSeek, None)
            .effective_base_url()
            .as_deref(),
        Some("https://api.deepseek.com/v1")
    );
    assert_eq!(
        profile("1", "a", ProviderKind::OpenAi, Some(" https://x.io/v1/ "))
            .effective_base_url()
            .as_deref(),
        Some("https://x.io/v1")
    );
    assert_eq!(
        profile("1", "a", ProviderKind::OpenAiCompatible, Some("  ")).effective_base_url(),
        None
    );
}

#[test]
fn validation_rules() {
    let mut s = Settings::with_defaults();
    s.profiles
        .push(profile("2", "DeepSeek", ProviderKind::DeepSeek, None));

    assert!(s
        .validate_profile(&profile("3", "Mới", ProviderKind::OpenAi, None))
        .is_ok());
    assert_eq!(
        s.validate_profile(&profile("3", "  ", ProviderKind::OpenAi, None))
            .unwrap_err(),
        "Tên profile không được để trống"
    );
    assert_eq!(
        s.validate_profile(&profile("3", "deepseek", ProviderKind::OpenAi, None))
            .unwrap_err(),
        "Tên profile đã tồn tại"
    );
    // same id may keep its own name
    assert!(s
        .validate_profile(&profile("2", "DeepSeek", ProviderKind::DeepSeek, None))
        .is_ok());
    assert_eq!(
        s.validate_profile(&profile("3", "X", ProviderKind::AnthropicCompatible, None))
            .unwrap_err(),
        "Base URL bắt buộc với provider tương thích"
    );
    let mut no_model = profile("3", "Y", ProviderKind::OpenAi, None);
    no_model.model = " ".into();
    assert_eq!(
        s.validate_profile(&no_model).unwrap_err(),
        "Model không được để trống"
    );
}

#[test]
fn find_profile_by_id_or_case_insensitive_name() {
    let mut s = Settings::with_defaults();
    s.profiles.push(profile(
        "abc",
        "DeepSeek Cá Nhân",
        ProviderKind::DeepSeek,
        None,
    ));
    assert_eq!(s.find_profile("abc").unwrap().name, "DeepSeek Cá Nhân");
    assert_eq!(s.find_profile("deepseek cá nhân").unwrap().id, "abc");
    assert!(s.find_profile("nope").is_none());
    assert_eq!(s.default_profile().unwrap().name, "Ollama local");
}

#[test]
fn unknown_fields_and_missing_sections_tolerated() {
    let s: Settings = serde_json::from_str(r#"{"version":1,"future":true}"#).unwrap();
    assert!(s.profiles.is_empty());
    assert_eq!(s.general, GeneralSettings::default());
}

#[test]
fn download_dir_defaults_to_none_and_roundtrips() {
    // None means the backend falls back to ~/Echo Studio/input/
    assert_eq!(GeneralSettings::default().download_dir, None);

    let s: Settings = serde_json::from_str(r#"{"version":1}"#).unwrap();
    assert_eq!(s.general.download_dir, None);

    let json = r#"{"general":{"download_dir":"/tmp/ytdl"}}"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    assert_eq!(s.general.download_dir.as_deref(), Some("/tmp/ytdl"));
    assert!(serde_json::to_string(&s.general)
        .unwrap()
        .contains("/tmp/ytdl"));
}
