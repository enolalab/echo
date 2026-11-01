pub mod error;
pub mod openai;

pub use error::LlmError;

use serde::de::DeserializeOwned;

#[async_trait::async_trait]
pub trait ChatClient: Send + Sync {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError>;
    async fn list_models(&self) -> Result<Vec<String>, LlmError>;
}

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
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
