pub mod llm_translator;
pub mod prompt;

pub use llm_translator::LlmTranslator;

use crate::subtitle::model::SubtitleSegment;

#[async_trait::async_trait]
pub trait Translator: Send + Sync {
    /// MUST return exactly segments.len() strings, in order, or Err.
    async fn translate_batch(
        &self,
        segments: &[SubtitleSegment],
        source_lang: &str,
        target_lang: &str,
    ) -> anyhow::Result<Vec<String>>;
}
