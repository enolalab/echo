use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::path::Path;

use super::Transcriber;
use crate::subtitle::model::{sanitize_segments, SubtitleSegment};

pub const API_MAX_UPLOAD_BYTES: u64 = 25 * 1024 * 1024;

pub struct ApiTranscriber {
    api_key: String,
    base_url: String,
    model: String,
    language: Option<String>,
    client: reqwest::Client,
}

impl ApiTranscriber {
    pub fn new(api_key: String, base_url: String, model: String, language: Option<String>) -> Self {
        Self {
            api_key,
            base_url,
            model,
            language,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct VerboseJsonSegment {
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct VerboseJsonResponse {
    #[serde(default)]
    segments: Option<Vec<VerboseJsonSegment>>,
}

#[async_trait]
impl Transcriber for ApiTranscriber {
    async fn transcribe(&self, audio_path: &Path) -> Result<Vec<SubtitleSegment>> {
        let metadata = tokio::fs::metadata(audio_path)
            .await
            .with_context(|| format!("failed to read metadata for {}", audio_path.display()))?;

        if metadata.len() > API_MAX_UPLOAD_BYTES {
            bail!(
                "file size ({} bytes) exceeds maximum upload limit of {} bytes (25MB); consider using '--stt-engine local'",
                metadata.len(),
                API_MAX_UPLOAD_BYTES
            );
        }

        let file_bytes = tokio::fs::read(audio_path)
            .await
            .with_context(|| format!("failed to read audio file at {}", audio_path.display()))?;

        let ext = audio_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let mime = match ext.to_lowercase().as_str() {
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "m4a" => "audio/m4a",
            "ogg" => "audio/ogg",
            "webm" => "audio/webm",
            "flac" => "audio/flac",
            _ => "application/octet-stream",
        };

        let file_name = audio_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav")
            .to_string();

        let file_part = Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str(mime)
            .context("invalid mime type")?;

        let mut form = Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", "verbose_json");

        if let Some(lang) = &self.language {
            form = form.text("language", lang.clone());
        }

        let url = format!(
            "{}/audio/transcriptions",
            self.base_url.trim_end_matches('/')
        );
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("failed to send transcription request to {url}"))?
            .error_for_status()
            .with_context(|| format!("transcription request failed at {url}"))?;

        let parsed: VerboseJsonResponse = resp
            .json()
            .await
            .context("failed to parse verbose_json response from transcription API")?;

        let segments = match parsed.segments {
            Some(list) => {
                let raw_segs = list
                    .into_iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let start_ms = (s.start * 1000.0).round().max(0.0) as u64;
                        let end_ms = (s.end * 1000.0).round().max(0.0) as u64;
                        SubtitleSegment::new(i + 1, start_ms, end_ms, s.text)
                    })
                    .collect();
                sanitize_segments(raw_segs)
            }
            None => Vec::new(),
        };

        Ok(segments)
    }
}
