use serde::Deserialize;
use serde_json::json;

use super::{http_client, send_json, ChatClient, LlmError};

pub struct OllamaChatClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaChatClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self { base_url: base_url.trim_end_matches('/').to_string(), model, client: http_client() }
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagItem>,
}

#[derive(Deserialize)]
struct TagItem {
    name: String,
}

#[async_trait::async_trait]
impl ChatClient for OllamaChatClient {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError> {
        let url = format!("{}/api/chat", self.base_url);
        let mut messages = Vec::new();
        if let Some(sys) = system {
            messages.push(json!({"role": "system", "content": sys}));
        }
        messages.push(json!({"role": "user", "content": user}));
        let body = json!({
            "model": self.model,
            "stream": false,
            "keep_alive": "30m",
            "options": {"temperature": 0.2, "num_ctx": 4096},
            "messages": messages
        });
        let resp: ChatResponse = send_json(self.client.post(&url).json(&body), &url).await?;
        Ok(resp.message.content)
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let url = format!("{}/api/tags", self.base_url);
        let resp: TagsResponse = send_json(self.client.get(&url), &url).await?;
        let mut names: Vec<String> = resp.models.into_iter().map(|m| m.name).collect();
        names.sort();
        Ok(names)
    }
}

pub fn ollama_model_present(models: &[String], model: &str) -> bool {
    models.iter().any(|m| {
        m == model
            || (!model.contains(':') && *m == format!("{model}:latest"))
            || (model.ends_with(":latest") && m == model.trim_end_matches(":latest"))
    })
}

pub async fn ensure_ollama_model(
    client: &dyn ChatClient,
    base_url: &str,
    model: &str,
) -> anyhow::Result<()> {
    let models = client.list_models().await.map_err(|e| {
        anyhow::anyhow!(
            "Không kết nối được với Ollama tại {base_url}. Hãy kiểm tra xem Ollama đã chạy chưa (chạy 'ollama serve') hoặc dùng --translate-engine openai-compat. ({e})"
        )
    })?;
    if !ollama_model_present(&models, model) {
        anyhow::bail!("Model '{model}' không tìm thấy trong Ollama. Hãy chạy 'ollama pull {model}' trước.");
    }
    Ok(())
}
