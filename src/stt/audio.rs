use anyhow::{bail, Context, Result};
use hound::{SampleFormat, WavReader};
use std::path::Path;

/// Err unless 16000 Hz, 1 channel, 16-bit int PCM. Samples scaled to [-1,1] by /32768.0.
pub fn read_wav_16k_mono(path: &Path) -> Result<Vec<f32>> {
    let mut reader = WavReader::open(path)
        .with_context(|| format!("failed to open WAV file at {}", path.display()))?;
    let spec = reader.spec();
    if spec.sample_rate != 16000 {
        bail!("expected sample rate 16000 Hz, got {}", spec.sample_rate);
    }
    if spec.channels != 1 {
        bail!("expected 1 channel (mono), got {}", spec.channels);
    }
    if spec.bits_per_sample != 16 || spec.sample_format != SampleFormat::Int {
        bail!(
            "expected 16-bit signed integer PCM, got {}-bit {:?}",
            spec.bits_per_sample,
            spec.sample_format
        );
    }

    let samples: Result<Vec<f32>, _> = reader
        .samples::<i16>()
        .map(|s| s.map(|val| val as f32 / 32768.0))
        .collect();

    let samples = samples.with_context(|| format!("failed to read audio samples from {}", path.display()))?;
    Ok(samples)
}
