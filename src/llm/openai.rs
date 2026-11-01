use serde::Deserialize;
use serde_json::json;

use super::{http_client, send_json, ChatClient, LlmError};

pub struct OpenAiChatClient {
    base_url: String,
    api_key: Option<String>,
    model: String,
    client: reqwest::Client,
}

impl OpenAiChatClient {
    pub fn new(base_url: String, api_key: Option<String>, model: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
            client: http_client(),
        }
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(k) => req.bearer_auth(k),
            None => req,
        }
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelItem>,
}

#[derive(Deserialize)]
struct ModelItem {
    id: String,
}

#[async_trait::async_trait]
impl ChatClient for OpenAiChatClient {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut messages = Vec::new();
        if let Some(sys) = system {
            messages.push(json!({"role": "system", "content": sys}));
        }
        messages.push(json!({"role": "user", "content": user}));
        let body = json!({"model": self.model, "temperature": 0.2, "messages": messages});
        let resp: ChatResponse = send_json(self.auth(self.client.post(&url)).json(&body), &url).await?;
        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content.unwrap_or_default())
            .ok_or_else(|| LlmError::Other { status: None, body: "Phản hồi không có choices".into() })
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let url = format!("{}/models", self.base_url);
        let resp: ModelsResponse = send_json(self.auth(self.client.get(&url)), &url).await?;
        let mut ids: Vec<String> = resp.data.into_iter().map(|m| m.id).collect();
        ids.sort();
        Ok(ids)
    }
}
