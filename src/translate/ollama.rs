use anyhow::Context;
use serde::Deserialize;

use crate::subtitle::model::SubtitleSegment;
use crate::translate::prompt::{build_translation_prompt, parse_translated_lines};
use crate::translate::Translator;

pub struct OllamaTranslator {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<OllamaModelItem>,
}

#[derive(Deserialize)]
struct OllamaModelItem {
    name: String,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaChatMessage,
}

#[derive(Deserialize)]
struct OllamaChatMessage {
    #[serde(default)]
    content: String,
}

impl OllamaTranslator {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url,
            model,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    /// GET {base}/api/tags. Unreachable => Err("Không kết nối được với Ollama ... chạy 'ollama serve' hoặc dùng --translate-engine openai-compat").
    /// Model missing from tags list (match "name" exact, or "name:latest" when no tag given) => Err("... chạy 'ollama pull <model>'").
    pub async fn health_check(&self) -> anyhow::Result<()> {
        let base = self.base_url.trim_end_matches('/');
        let url = format!("{base}/api/tags");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| {
                format!(
                    "Không kết nối được với Ollama tại {base}. Hãy kiểm tra xem Ollama đã chạy chưa (chạy 'ollama serve') hoặc dùng --translate-engine openai-compat."
                )
            })?
            .error_for_status()
            .with_context(|| format!("Ollama tại {base} trả về mã lỗi HTTP"))?;

        let tags: TagsResponse = resp
            .json()
            .await
            .context("Không thể phân tích phản hồi từ Ollama /api/tags")?;

        let model_name = &self.model;
        let found = tags.models.iter().any(|m| {
            if m.name == *model_name {
                return true;
            }
            if !model_name.contains(':') && m.name == format!("{model_name}:latest") {
                return true;
            }
            if model_name.ends_with(":latest") && m.name == model_name.trim_end_matches(":latest") {
                return true;
            }
            false
        });

        if !found {
            anyhow::bail!(
                "Model '{}' không tìm thấy trong Ollama. Hãy chạy 'ollama pull {}' trước.",
                self.model,
                self.model
            );
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Translator for OllamaTranslator {
    async fn translate_batch(
        &self,
        segments: &[SubtitleSegment],
        source_lang: &str,
        target_lang: &str,
    ) -> anyhow::Result<Vec<String>> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }

        let prompt = build_translation_prompt(segments, source_lang, target_lang);
        let base = self.base_url.trim_end_matches('/');
        let url = format!("{base}/api/chat");

        let body = serde_json::json!({
            "model": self.model,
            "stream": false,
            "options": {
                "temperature": 0.2
            },
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Không thể gửi yêu cầu dịch tới Ollama tại {url}"))?
            .error_for_status()
            .with_context(|| format!("Ollama tại {url} trả về lỗi HTTP"))?;

        let chat_resp: OllamaChatResponse = resp
            .json()
            .await
            .context("Không thể giải mã phản hồi JSON từ Ollama /api/chat")?;

        let translated = parse_translated_lines(&chat_resp.message.content, segments.len())
            .with_context(|| "Không thể phân tích dòng dịch từ phản hồi của Ollama")?;

        Ok(translated)
    }
}
