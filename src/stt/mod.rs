use crate::subtitle::model::SubtitleSegment;
use std::path::Path;

pub mod audio;
pub mod whisper_api;
pub mod whisper_local;

#[async_trait::async_trait]
pub trait Transcriber: Send + Sync {
    /// Returns sanitized segments (ids 1..=n). Err on engine failure; Ok(vec![]) means no speech.
    async fn transcribe(&self, audio_path: &Path) -> anyhow::Result<Vec<SubtitleSegment>>;
}
