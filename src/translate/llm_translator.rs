use crate::llm::ChatClient;
use crate::subtitle::model::SubtitleSegment;
use crate::translate::prompt::{build_translation_prompt, parse_translated_lines};
use crate::translate::Translator;

pub struct LlmTranslator {
    client: Box<dyn ChatClient>,
    system_prompt: Option<String>,
}

impl LlmTranslator {
    pub fn new(client: Box<dyn ChatClient>, system_prompt: Option<String>) -> Self {
        Self { client, system_prompt }
    }
}

#[async_trait::async_trait]
impl Translator for LlmTranslator {
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
        let raw = self
            .client
            .chat(self.system_prompt.as_deref(), &prompt)
            .await
            .map_err(anyhow::Error::new)?;
        parse_translated_lines(&raw, segments.len())
    }
}
