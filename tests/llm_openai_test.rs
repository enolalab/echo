use auto_subtitle::llm::{openai::OpenAiChatClient, ChatClient, LlmError};
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn chat_sends_bearer_system_and_user() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer k"))
        .and(body_partial_json(json!({
            "model": "m",
            "messages": [
                {"role": "system", "content": "SYS"},
                {"role": "user", "content": "hi"}
            ]
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"choices":[{"message":{"content":"hello"}}]})),
        )
        .mount(&s)
        .await;
    let c = OpenAiChatClient::new(format!("{}/v1", s.uri()), Some("k".into()), "m".into());
    assert_eq!(c.chat(Some("SYS"), "hi").await.unwrap(), "hello");
}

#[tokio::test]
async fn chat_without_system_sends_only_user() {
    let s = MockServer::start().await;
    Mock::given(path("/chat/completions"))
        .and(body_partial_json(
            json!({"messages":[{"role":"user","content":"hi"}]}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"choices":[{"message":{"content":"ok"}}]})),
        )
        .mount(&s)
        .await;
    let c = OpenAiChatClient::new(s.uri(), None, "m".into());
    assert_eq!(c.chat(None, "hi").await.unwrap(), "ok");
}

#[tokio::test]
async fn list_models_sorted() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"data":[{"id":"gpt-4o"},{"id":"deepseek-chat"}]})),
        )
        .mount(&s)
        .await;
    let c = OpenAiChatClient::new(s.uri(), Some("k".into()), "m".into());
    assert_eq!(
        c.list_models().await.unwrap(),
        vec!["deepseek-chat", "gpt-4o"]
    );
}

#[tokio::test]
async fn status_mapping() {
    for (code, expected) in [
        (401u16, LlmError::Auth),
        (403, LlmError::Auth),
        (404, LlmError::NotFound),
        (429, LlmError::RateLimit),
    ] {
        let s = MockServer::start().await;
        Mock::given(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(code))
            .mount(&s)
            .await;
        let c = OpenAiChatClient::new(s.uri(), Some("k".into()), "m".into());
        assert_eq!(c.chat(None, "x").await.unwrap_err(), expected);
    }

    let s = MockServer::start().await;
    Mock::given(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("x".repeat(300)))
        .mount(&s)
        .await;
    let c = OpenAiChatClient::new(s.uri(), Some("k".into()), "m".into());
    match c.chat(None, "x").await.unwrap_err() {
        LlmError::Other { status, body } => {
            assert_eq!(status, Some(500));
            assert_eq!(body.len(), 200);
        }
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn empty_choices_is_retryable_other() {
    let s = MockServer::start().await;
    Mock::given(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices":[]})))
        .mount(&s)
        .await;
    let c = OpenAiChatClient::new(s.uri(), None, "m".into());
    let e = c.chat(None, "x").await.unwrap_err();
    assert!(matches!(e, LlmError::Other { status: None, .. }));
    assert!(e.is_retryable());
}

#[tokio::test]
async fn unreachable_host_is_network_error() {
    let c = OpenAiChatClient::new("http://127.0.0.1:9".into(), None, "m".into());
    let e = c.chat(None, "x").await.unwrap_err();
    assert_eq!(e, LlmError::Network("127.0.0.1".into()));
    assert_eq!(e.user_message(), "Không kết nối được tới 127.0.0.1");
}

#[test]
fn retry_classification_and_messages() {
    assert!(!LlmError::Auth.is_retryable());
    assert!(!LlmError::NotFound.is_retryable());
    assert!(!LlmError::Config("x".into()).is_retryable());
    assert!(!LlmError::Other {
        status: Some(400),
        body: String::new()
    }
    .is_retryable());
    assert!(LlmError::RateLimit.is_retryable() && LlmError::RateLimit.should_backoff());
    let e500 = LlmError::Other {
        status: Some(503),
        body: String::new(),
    };
    assert!(e500.is_retryable() && e500.should_backoff());
    assert!(LlmError::Timeout("h".into()).is_retryable());
    assert!(!LlmError::Timeout("h".into()).should_backoff());

    assert_eq!(LlmError::Auth.user_message(), "API key không hợp lệ");
    assert_eq!(
        LlmError::NotFound.user_message(),
        "Sai base URL hoặc tên model"
    );
    assert_eq!(
        LlmError::RateLimit.user_message(),
        "Bị giới hạn tốc độ, thử lại sau"
    );
    assert_eq!(
        LlmError::Timeout("h".into()).user_message(),
        "Không kết nối được tới h"
    );
    assert_eq!(LlmError::Config("Thiếu".into()).user_message(), "Thiếu");
    assert_eq!(
        LlmError::Other {
            status: Some(500),
            body: "boom".into()
        }
        .user_message(),
        "Lỗi provider (500): boom"
    );
    assert_eq!(
        LlmError::Other {
            status: None,
            body: "bad".into()
        }
        .user_message(),
        "Lỗi provider: bad"
    );
    assert_eq!(LlmError::Auth.to_string(), "API key không hợp lệ");
}
