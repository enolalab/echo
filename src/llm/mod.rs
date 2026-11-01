pub mod anthropic;
pub mod error;
pub mod ollama;
pub mod openai;

pub use error::LlmError;

use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::settings::{ProviderKind, ProviderProfile};

pub const TEST_TIMEOUT: Duration = Duration::from_secs(20);

#[async_trait::async_trait]
pub trait ChatClient: Send + Sync {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError>;
    async fn list_models(&self) -> Result<Vec<String>, LlmError>;
}

pub fn build_chat_client(
    profile: &ProviderProfile,
    api_key: Option<String>,
) -> Result<Box<dyn ChatClient>, LlmError> {
    let base = profile
        .effective_base_url()
        .ok_or_else(|| LlmError::Config("Base URL bắt buộc với provider tương thích".into()))?;
    let model = profile.model.clone();
    let key = api_key.filter(|k| !k.trim().is_empty());
    Ok(match profile.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible | ProviderKind::DeepSeek => {
            Box::new(openai::OpenAiChatClient::new(base, key, model))
        }
        ProviderKind::Anthropic | ProviderKind::AnthropicCompatible => {
            Box::new(anthropic::AnthropicChatClient::new(base, key, model))
        }
        ProviderKind::Ollama => Box::new(ollama::OllamaChatClient::new(base, model)),
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConnectionTest {
    pub ok: bool,
    pub latency_ms: u64,
    pub error: Option<String>,
}

pub async fn test_connection(client: &dyn ChatClient, timeout: Duration) -> ConnectionTest {
    let start = Instant::now();
    let res = tokio::time::timeout(timeout, client.chat(None, "Reply with: OK")).await;
    let latency_ms = start.elapsed().as_millis() as u64;
    let error = match res {
        Ok(Ok(_)) => None,
        Ok(Err(e)) => Some(e.user_message()),
        Err(_) => Some("Hết thời gian chờ".to_string()),
    };
    ConnectionTest { ok: error.is_none(), latency_ms, error }
}

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub(crate) async fn send_json<T: DeserializeOwned>(
    req: reqwest::RequestBuilder,
    url: &str,
) -> Result<T, LlmError> {
    let resp = req.send().await.map_err(|e| LlmError::from_reqwest(&e, url))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(LlmError::from_status(status.as_u16(), body));
    }
    resp.json::<T>().await.map_err(|e| LlmError::Other {
        status: None,
        body: format!("Phản hồi không hợp lệ: {e}"),
    })
}
