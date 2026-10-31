use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

use super::audio::read_wav_16k_mono;
use super::Transcriber;
use crate::subtitle::model::{sanitize_segments, SubtitleSegment};

pub struct LocalWhisperTranscriber {
    model_path: PathBuf,
    language: Option<String>,
    use_gpu: bool,
}

impl LocalWhisperTranscriber {
    pub fn new(model_path: PathBuf, language: Option<String>, use_gpu: bool) -> Self {
        Self {
            model_path,
            language,
            use_gpu,
        }
    }
}

/// (t0_centiseconds, t1_centiseconds, text) -> segments in ms, then sanitize_segments.
pub fn segments_from_whisper(raw: Vec<(i64, i64, String)>) -> Vec<SubtitleSegment> {
    let segs = raw
        .into_iter()
        .enumerate()
        .map(|(i, (t0, t1, text))| {
            let start_ms = (t0.max(0) as u64) * 10;
            let end_ms = (t1.max(0) as u64) * 10;
            SubtitleSegment::new(i + 1, start_ms, end_ms, text)
        })
        .collect();
    sanitize_segments(segs)
}

#[async_trait]
impl Transcriber for LocalWhisperTranscriber {
    async fn transcribe(&self, audio_path: &Path) -> Result<Vec<SubtitleSegment>> {
        let samples = read_wav_16k_mono(audio_path)?;
        let (model, lang, gpu) = (
            self.model_path.clone(),
            self.language.clone(),
            self.use_gpu,
        );
        tokio::task::spawn_blocking(move || {
            match run_whisper(&model, lang.as_deref(), &samples, gpu) {
                Ok(raw) => Ok(raw),
                Err(e) if gpu => {
                    eprintln!("⚠️ GPU inference failed ({e:#}); retrying on CPU...");
                    run_whisper(&model, lang.as_deref(), &samples, false)
                }
                Err(e) => Err(e),
            }
        })
        .await
        .context("transcription task panicked")?
        .map(segments_from_whisper)
    }
}

fn run_whisper(
    model: &Path,
    lang: Option<&str>,
    samples: &[f32],
    use_gpu: bool,
) -> Result<Vec<(i64, i64, String)>> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    let mut cp = WhisperContextParameters::default();
    cp.use_gpu(use_gpu);
    let model_str = model.to_str().context("model path is not valid UTF-8")?;
    let ctx = WhisperContext::new_with_params(model_str, cp).context("load whisper model")?;
    let mut state = ctx.create_state().context("create whisper state")?;
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    p.set_language(Some(lang.unwrap_or("auto")));
    p.set_n_threads(
        std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4),
    );
    p.set_print_progress(false);
    p.set_print_realtime(false);
    p.set_print_special(false);
    p.set_print_timestamps(false);
    state.full(p, samples).context("whisper inference")?;
    let n = state.full_n_segments().context("get segment count")?;
    (0..n)
        .map(|i| {
            let t0 = state
                .full_get_segment_t0(i)
                .context("get segment start")?;
            let t1 = state.full_get_segment_t1(i).context("get segment end")?;
            let text = state
                .full_get_segment_text(i)
                .context("get segment text")?;
            Ok((t0, t1, text))
        })
        .collect()
}
