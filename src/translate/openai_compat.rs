use anyhow::Context;
use serde::Deserialize;

use crate::subtitle::model::SubtitleSegment;
use crate::translate::prompt::{build_translation_prompt, parse_translated_lines};
use crate::translate::Translator;

pub struct OpenAiCompatTranslator {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    #[serde(default)]
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Deserialize)]
struct OpenAiMessage {
    #[serde(default)]
    content: Option<String>,
}

impl OpenAiCompatTranslator {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        Self {
            base_url,
            api_key,
            model,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

#[async_trait::async_trait]
impl Translator for OpenAiCompatTranslator {
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
        let url = format!("{base}/chat/completions");

        let body = serde_json::json!({
            "model": self.model,
            "temperature": 0.2,
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
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Không thể gửi yêu cầu dịch tới {url}"))?
            .error_for_status()
            .with_context(|| format!("Dịch vụ OpenAI-compatible tại {url} trả về lỗi HTTP"))?;

        let chat_resp: OpenAiChatResponse = resp
            .json()
            .await
            .context("Không thể giải mã phản hồi JSON từ /chat/completions")?;

        let choice = chat_resp.choices.first().ok_or_else(|| {
            anyhow::anyhow!("Phản hồi từ OpenAI-compatible không chứa lựa chọn nào (choices rỗng)")
        })?;

        let raw_content = choice.message.content.as_deref().unwrap_or_default();
        let translated = parse_translated_lines(raw_content, segments.len())
            .with_context(|| "Không thể phân tích dòng dịch từ phản hồi của OpenAI-compatible")?;

        Ok(translated)
    }
}
