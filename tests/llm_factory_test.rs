use auto_subtitle::llm::ollama::{ensure_ollama_model, ollama_model_present, OllamaChatClient};
use auto_subtitle::llm::{build_chat_client, test_connection, ChatClient};
use auto_subtitle::llm::LlmError;
use auto_subtitle::settings::{ProviderKind, ProviderProfile};
use serde_json::json;
use std::time::Duration;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn profile(kind: ProviderKind, base: Option<String>) -> ProviderProfile {
    ProviderProfile { id: "1".into(), name: "p".into(), kind, base_url: base, model: "m".into() }
}

#[tokio::test]
async fn ollama_chat_and_tags() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_partial_json(json!({
            "model": "m",
            "stream": false,
            "messages": [{"role":"system","content":"S"},{"role":"user","content":"u"}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"message":{"content":"r"}})))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"models":[{"name":"qwen2.5:7b"},{"name":"llama3:latest"}]}),
        ))
        .mount(&s)
        .await;
    let c = OllamaChatClient::new(s.uri(), "m".into());
    assert_eq!(c.chat(Some("S"), "u").await.unwrap(), "r");
    assert_eq!(c.list_models().await.unwrap(), vec!["llama3:latest", "qwen2.5:7b"]);
    ensure_ollama_model(&c, &s.uri(), "llama3").await.unwrap();
    let err = ensure_ollama_model(&c, &s.uri(), "mistral").await.unwrap_err();
    assert!(format!("{err:#}").contains("ollama pull mistral"));
}

#[tokio::test]
async fn ollama_unreachable_is_actionable() {
    let c = OllamaChatClient::new("http://127.0.0.1:9".into(), "m".into());
    let err = ensure_ollama_model(&c, "http://127.0.0.1:9", "m").await.unwrap_err();
    assert!(format!("{err:#}").contains("ollama serve"));
}

#[test]
fn model_presence_rules() {
    let tags = vec!["qwen2.5:latest".to_string(), "llama3:8b".to_string()];
    assert!(ollama_model_present(&tags, "qwen2.5"));
    assert!(ollama_model_present(&tags, "qwen2.5:latest"));
    assert!(ollama_model_present(&tags, "llama3:8b"));
    assert!(!ollama_model_present(&tags, "llama3"));
    assert!(ollama_model_present(&["mistral".to_string()], "mistral:latest"));
}

#[tokio::test]
async fn factory_routes_kinds_to_protocols() {
    let s = MockServer::start().await;
    Mock::given(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices":[{"message":{"content":"openai"}}]})))
        .mount(&s)
        .await;
    Mock::given(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"content":[{"type":"text","text":"anthropic"}]})))
        .mount(&s)
        .await;
    Mock::given(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"message":{"content":"ollama"}})))
        .mount(&s)
        .await;

    for (kind, expected) in [
        (ProviderKind::OpenAi, "openai"),
        (ProviderKind::OpenAiCompatible, "openai"),
        (ProviderKind::DeepSeek, "openai"),
        (ProviderKind::Anthropic, "anthropic"),
        (ProviderKind::AnthropicCompatible, "anthropic"),
        (ProviderKind::Ollama, "ollama"),
    ] {
        let c = build_chat_client(&profile(kind, Some(s.uri())), Some("k".into())).unwrap();
        assert_eq!(c.chat(None, "x").await.unwrap(), expected, "{kind:?}");
    }
}

#[test]
fn factory_rejects_compatible_without_base_url() {
    let err = build_chat_client(&profile(ProviderKind::OpenAiCompatible, None), None).err().unwrap();
    assert_eq!(err, LlmError::Config("Base URL bắt buộc với provider tương thích".into()));
}

#[tokio::test]
async fn connection_test_ok_error_and_timeout() {
    let s = MockServer::start().await;
    Mock::given(path("/ok/chat/completions"))
        .and(body_partial_json(json!({"messages":[{"role":"user","content":"Reply with: OK"}]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices":[{"message":{"content":"OK"}}]})))
        .mount(&s)
        .await;
    Mock::given(path("/bad/chat/completions"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&s)
        .await;
    Mock::given(path("/slow/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(500))
                .set_body_json(json!({"choices":[{"message":{"content":"OK"}}]})),
        )
        .mount(&s)
        .await;

    let ok = build_chat_client(&profile(ProviderKind::OpenAi, Some(format!("{}/ok", s.uri()))), None).unwrap();
    let r = test_connection(ok.as_ref(), Duration::from_secs(5)).await;
    assert!(r.ok && r.error.is_none());

    let bad = build_chat_client(&profile(ProviderKind::OpenAi, Some(format!("{}/bad", s.uri()))), None).unwrap();
    let r = test_connection(bad.as_ref(), Duration::from_secs(5)).await;
    assert!(!r.ok);
    assert_eq!(r.error.as_deref(), Some("API key không hợp lệ"));

    let slow = build_chat_client(&profile(ProviderKind::OpenAi, Some(format!("{}/slow", s.uri()))), None).unwrap();
    let r = test_connection(slow.as_ref(), Duration::from_millis(50)).await;
    assert!(!r.ok);
    assert_eq!(r.error.as_deref(), Some("Hết thời gian chờ"));
}
