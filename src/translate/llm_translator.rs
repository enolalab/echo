use crate::llm::ChatClient;
use crate::skills::prompt::render_system_prompt;
use crate::skills::Skill;
use crate::subtitle::model::SubtitleSegment;
use crate::translate::prompt::{build_translation_prompt, parse_translated_lines};
use crate::translate::Translator;

pub struct LlmTranslator {
    client: Box<dyn ChatClient>,
    skill: Option<Skill>,
}

impl LlmTranslator {
    pub fn new(client: Box<dyn ChatClient>, skill: Option<Skill>) -> Self {
        Self { client, skill }
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
        let system = self
            .skill
            .as_ref()
            .map(|s| render_system_prompt(s, segments, source_lang, target_lang));
        let prompt = build_translation_prompt(segments, source_lang, target_lang);
        let raw = self
            .client
            .chat(system.as_deref(), &prompt)
            .await
            .map_err(anyhow::Error::new)?;
        parse_translated_lines(&raw, segments.len())
    }
}
