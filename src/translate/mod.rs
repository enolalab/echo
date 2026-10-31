pub mod ollama;
pub mod openai_compat;
pub mod prompt;

pub use ollama::OllamaTranslator;
pub use openai_compat::OpenAiCompatTranslator;

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
