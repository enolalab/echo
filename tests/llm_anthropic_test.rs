use auto_subtitle::llm::{anthropic::AnthropicChatClient, ChatClient, LlmError};
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn chat_uses_messages_api_headers_and_top_level_system() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "k"))
        .and(header("anthropic-version", "2023-06-01"))
        .and(body_partial_json(json!({
            "model": "claude-x",
            "max_tokens": 8192,
            "system": "SYS",
            "messages": [{"role": "user", "content": "hi"}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [
                {"type": "text", "text": "a"},
                {"type": "tool_use", "id": "t"},
                {"type": "text", "text": "b"}
            ]
        })))
        .mount(&s)
        .await;
    let c = AnthropicChatClient::new(s.uri(), Some("k".into()), "claude-x".into());
    assert_eq!(c.chat(Some("SYS"), "hi").await.unwrap(), "ab");
}

#[tokio::test]
async fn chat_without_system_omits_field() {
    let s = MockServer::start().await;
    Mock::given(path("/v1/messages"))
        .respond_with(move |req: &wiremock::Request| {
            let v: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            assert!(v.get("system").is_none());
            ResponseTemplate::new(200).set_body_json(json!({"content":[{"type":"text","text":"ok"}]}))
        })
        .mount(&s)
        .await;
    let c = AnthropicChatClient::new(format!("{}/", s.uri()), Some("k".into()), "m".into());
    assert_eq!(c.chat(None, "hi").await.unwrap(), "ok");
}

#[tokio::test]
async fn list_models_and_auth_error() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header("x-api-key", "k"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"data":[{"id":"claude-b"},{"id":"claude-a"}]}),
        ))
        .mount(&s)
        .await;
    Mock::given(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&s)
        .await;
    let c = AnthropicChatClient::new(s.uri(), Some("k".into()), "m".into());
    assert_eq!(c.list_models().await.unwrap(), vec!["claude-a", "claude-b"]);
    assert_eq!(c.chat(None, "x").await.unwrap_err(), LlmError::Auth);
}
