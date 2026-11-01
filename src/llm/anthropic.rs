use serde::Deserialize;
use serde_json::json;

use super::{http_client, send_json, ChatClient, LlmError};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 8192;

pub struct AnthropicChatClient {
    base_url: String,
    api_key: Option<String>,
    model: String,
    client: reqwest::Client,
}

impl AnthropicChatClient {
    pub fn new(base_url: String, api_key: Option<String>, model: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
            client: http_client(),
        }
    }

    fn headers(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let req = req.header("anthropic-version", ANTHROPIC_VERSION);
        match &self.api_key {
            Some(k) => req.header("x-api-key", k),
            None => req,
        }
    }
}

#[derive(Deserialize)]
struct MessagesResponse {
    #[serde(default)]
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
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
impl ChatClient for AnthropicChatClient {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError> {
        let url = format!("{}/v1/messages", self.base_url);
        let mut body = json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "temperature": 0.2,
            "messages": [{"role": "user", "content": user}]
        });
        if let Some(sys) = system {
            body["system"] = json!(sys);
        }
        let resp: MessagesResponse =
            send_json(self.headers(self.client.post(&url)).json(&body), &url).await?;
        Ok(resp
            .content
            .into_iter()
            .filter(|b| b.kind == "text")
            .filter_map(|b| b.text)
            .collect())
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let url = format!("{}/v1/models", self.base_url);
        let resp: ModelsResponse = send_json(self.headers(self.client.get(&url)), &url).await?;
        let mut ids: Vec<String> = resp.data.into_iter().map(|m| m.id).collect();
        ids.sort();
        Ok(ids)
    }
}
