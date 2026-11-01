use auto_subtitle::cli::*;
use auto_subtitle::settings::{ProviderKind, ProviderProfile, Settings};
use clap::Parser;

#[test]
fn defaults_match_spec() {
    let c = Cli::try_parse_from(["auto-subtitle", "-i", "v.mp4"]).unwrap();
    assert_eq!(c.target_lang, "vi");
    assert_eq!(c.source_lang, "auto");
    assert_eq!(c.mode, OutputMode::Hardsub);
    assert_eq!(c.stt_engine, SttEngine::Local);
    assert_eq!(c.whisper_model, "small");
    assert_eq!(c.translate_engine, None);
    assert_eq!(c.ollama_model, None);
    assert_eq!(c.profile, None);
    assert!(!c.list_profiles);
    assert_eq!(c.api_base_url_or_default(), "https://api.openai.com/v1");
    assert!(!c.bilingual && !c.skip_translate && !c.keep_temp);
}

#[test]
fn sub_alias_and_modes() {
    let c = Cli::try_parse_from([
        "auto-subtitle",
        "-i",
        "v.mp4",
        "--sub",
        "s.srt",
        "--mode",
        "subs-only",
        "--skip-translate",
        "-b",
    ])
    .unwrap();
    assert_eq!(c.subtitle.as_deref(), Some(std::path::Path::new("s.srt")));
    assert_eq!(c.mode, OutputMode::SubsOnly);
    assert!(c.skip_translate && c.bilingual);
}

#[test]
fn invalid_values_rejected() {
    assert!(Cli::try_parse_from(["auto-subtitle", "-i", "v.mp4", "--mode", "burn"]).is_err());
    assert!(Cli::try_parse_from(["auto-subtitle"]).is_err());
}

fn settings_with_deepseek() -> Settings {
    let mut s = Settings::with_defaults();
    s.profiles.push(ProviderProfile {
        id: "ds-1".into(),
        name: "DeepSeek".into(),
        kind: ProviderKind::DeepSeek,
        base_url: None,
        model: "deepseek-chat".into(),
    });
    s
}

fn parse(args: &[&str]) -> Cli {
    let mut full = vec!["auto-subtitle", "-i", "v.mp4"];
    full.extend_from_slice(args);
    Cli::try_parse_from(full).unwrap()
}

#[test]
fn explicit_engine_takes_legacy_path_even_with_profiles() {
    let s = settings_with_deepseek();
    let c = parse(&["--translate-engine", "openai-compat", "--profile", "DeepSeek"]);
    assert!(matches!(
        choose_translator(&c, Some(&s)).unwrap(),
        TranslatorChoice::Legacy(TranslateEngine::OpenaiCompat)
    ));
}

#[test]
fn profile_by_name_or_id_and_default() {
    let s = settings_with_deepseek();
    for q in ["deepseek", "ds-1"] {
        match choose_translator(&parse(&["--profile", q]), Some(&s)).unwrap() {
            TranslatorChoice::Profile(p) => assert_eq!(p.id, "ds-1"),
            _ => panic!("expected profile"),
        }
    }
    match choose_translator(&parse(&[]), Some(&s)).unwrap() {
        TranslatorChoice::Profile(p) => assert_eq!(p.name, "Ollama local"),
        _ => panic!("expected default profile"),
    }
}

#[test]
fn unknown_profile_and_missing_settings() {
    let s = settings_with_deepseek();
    let err = choose_translator(&parse(&["--profile", "claude"]), Some(&s)).unwrap_err();
    assert!(err.to_string().contains("Không tìm thấy profile 'claude'"));
    assert!(choose_translator(&parse(&["--profile", "x"]), None).is_err());
    assert!(matches!(
        choose_translator(&parse(&[]), None).unwrap(),
        TranslatorChoice::Legacy(TranslateEngine::Ollama)
    ));
    let empty = Settings { profiles: vec![], default_profile_id: None, ..Settings::with_defaults() };
    assert!(matches!(
        choose_translator(&parse(&[]), Some(&empty)).unwrap(),
        TranslatorChoice::Legacy(TranslateEngine::Ollama)
    ));
}

#[test]
fn list_profiles_flag_parses() {
    assert!(parse(&["--list-profiles"]).list_profiles);
}
