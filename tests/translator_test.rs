use auto_subtitle::subtitle::model::SubtitleSegment as S;
use auto_subtitle::translate::{
    ollama::OllamaTranslator, openai_compat::OpenAiCompatTranslator, prompt::*, Translator,
};
use serde_json::json;
use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};

fn batch() -> Vec<S> {
    vec![
        S::new(21, 0, 1000, "Good morning."),
        S::new(22, 1000, 2000, "How are\nyou?"),
    ]
}

#[test]
fn prompt_uses_local_numbering() {
    let p = build_translation_prompt(&batch(), "en", "vi");
    assert!(p.contains("[1] Good morning.") && p.contains("[2] How are / you?"));
    assert!(!p.contains("[21]"));
}

#[test]
fn prompt_handles_auto_source_lang() {
    let p = build_translation_prompt(&batch(), "auto", "vi");
    assert!(p.contains("Detect the source language automatically."));
}

#[test]
fn parse_ok_and_missing_line_errors() {
    let ok =
        parse_translated_lines("Here:\n[1] Chào buổi sáng.\n[2] Bạn / khỏe không?\n", 2).unwrap();
    assert_eq!(ok, vec!["Chào buổi sáng.", "Bạn\nkhỏe không?"]);
    assert!(parse_translated_lines("[1] only one", 2).is_err());
}

#[test]
fn parse_ignores_extras_and_out_of_range() {
    let raw =
        "Intro commentary\n[0] Zero\n[1] First\n[2] Second\n[3] Third extra\n[99] Ninety-nine";
    let ok = parse_translated_lines(raw, 2).unwrap();
    assert_eq!(ok, vec!["First", "Second"]);
}

#[tokio::test]
async fn ollama_translate_and_health() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"models":[{"name":"qwen2.5:7b"}]})),
        )
        .mount(&s)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"message":{"content":"[1] A\n[2] B"}})),
        )
        .mount(&s)
        .await;
    let t = OllamaTranslator::new(s.uri(), "qwen2.5:7b".into());
    t.health_check().await.unwrap();
    assert_eq!(
        t.translate_batch(&batch(), "en", "vi").await.unwrap(),
        vec!["A", "B"]
    );
    assert!(OllamaTranslator::new(s.uri(), "llama3".into())
        .health_check()
        .await
        .is_err());
}

#[tokio::test]
async fn ollama_health_matches_name_without_tag_to_latest() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"models":[{"name":"qwen2.5:latest"}]})),
        )
        .mount(&s)
        .await;
    let t = OllamaTranslator::new(s.uri(), "qwen2.5".into());
    t.health_check().await.unwrap();
}

#[tokio::test]
async fn ollama_unreachable_is_actionable_error() {
    let err = OllamaTranslator::new("http://127.0.0.1:9".into(), "x".into())
        .health_check()
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("ollama serve"));
}

#[tokio::test]
async fn openai_compat_sends_bearer_and_surfaces_http_errors() {
    let s = MockServer::start().await;
    Mock::given(path("/chat/completions"))
        .and(header("authorization", "Bearer k"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"choices":[{"message":{"content":"[1] A\n[2] B"}}]})),
        )
        .mount(&s)
        .await;
    let t = OpenAiCompatTranslator::new(s.uri(), "k".into(), "m".into());
    assert_eq!(
        t.translate_batch(&batch(), "en", "vi").await.unwrap().len(),
        2
    );
    let bad = OpenAiCompatTranslator::new(s.uri(), "wrong".into(), "m".into());
    assert!(bad.translate_batch(&batch(), "en", "vi").await.is_err()); // wiremock returns 404
}
