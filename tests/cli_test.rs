use auto_subtitle::cli::*;
use clap::Parser;

#[test]
fn defaults_match_spec() {
    let c = Cli::try_parse_from(["auto-subtitle", "-i", "v.mp4"]).unwrap();
    assert_eq!(c.target_lang, "vi");
    assert_eq!(c.source_lang, "auto");
    assert_eq!(c.mode, OutputMode::Hardsub);
    assert_eq!(c.stt_engine, SttEngine::Local);
    assert_eq!(c.whisper_model, "small");
    assert_eq!(c.translate_engine, TranslateEngine::Ollama);
    assert_eq!(c.ollama_model, "qwen2.5:7b");
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
