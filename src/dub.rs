use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::subtitle::model::SubtitleSegment;
use crate::tts::{normalize_tts_text, TtsEngine};

pub const MAX_OVERFLOW_MS: u64 = 2000;
pub const MAX_TEMPO: f64 = 1.5;

#[derive(Debug)]
pub struct DubPlan {
    pub seg_index: usize,
    pub start_ms: u64,
    pub slot_ms: u64,
    pub budget_ms: u64,
    pub text: String,
}

/// budget = slot + min(gap*0.5, MAX_OVERFLOW_MS); gap(last)=video_end-end else 0.
pub fn plan_segments(segs: &[SubtitleSegment], video_end_ms: Option<u64>) -> Vec<DubPlan> {
    segs.iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let raw = s.translated_text.as_deref().unwrap_or(&s.original_text);
            let text = normalize_tts_text(raw);
            if text.is_empty() {
                return None;
            }
            let slot = s.end_ms.saturating_sub(s.start_ms);
            let gap = if i + 1 < segs.len() {
                segs[i + 1].start_ms.saturating_sub(s.end_ms)
            } else {
                video_end_ms
                    .map(|v| v.saturating_sub(s.end_ms))
                    .unwrap_or(0)
            };
            Some(DubPlan {
                seg_index: i,
                start_ms: s.start_ms,
                slot_ms: slot,
                budget_ms: slot + std::cmp::min(gap / 2, MAX_OVERFLOW_MS),
                text,
            })
        })
        .collect()
}

/// 1.0 fits budget; else dur/budget clamped to [1.0, MAX_TEMPO].
pub fn decide_tempo(dur_ms: u64, budget_ms: u64) -> f64 {
    if dur_ms <= budget_ms {
        1.0
    } else {
        (dur_ms as f64 / budget_ms.max(1) as f64).clamp(1.0, MAX_TEMPO)
    }
}

/// "Duration: HH:MM:SS.cc" from `ffmpeg -i` stderr → ms.
pub fn parse_ffmpeg_duration_ms(stderr: &str) -> Option<u64> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re =
        RE.get_or_init(|| regex::Regex::new(r"Duration: (\d+):(\d+):(\d+)(?:\.(\d+))?").unwrap());
    let c = re.captures(stderr)?;
    let h: u64 = c[1].parse().ok()?;
    let m: u64 = c[2].parse().ok()?;
    let s: u64 = c[3].parse().ok()?;
    let frac: u64 = c.get(4).map_or(0, |f| {
        let t = f.as_str();
        t[..t.len().min(3)].parse::<u64>().unwrap_or(0) * 10u64.pow(3 - t.len().min(3) as u32)
    });
    Some(h * 3_600_000 + m * 60_000 + s * 1000 + frac)
}

pub fn parse_has_audio(stderr: &str) -> bool {
    stderr.contains("Audio:")
}

/// Runs `ffmpeg -i <video>`; stderr carries stream info. (duration_ms, has_audio)
pub fn probe_media(ffmpeg: &Path, video: &Path) -> (Option<u64>, bool) {
    let out = std::process::Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(video)
        .output();
    match out {
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            (parse_ffmpeg_duration_ms(&err), parse_has_audio(&err))
        }
        Err(_) => (None, false),
    }
}

#[derive(Debug)]
pub struct DubClip {
    pub start_ms: u64,
    pub wav_path: PathBuf, // normalized 48 kHz mono s16
}

/// Sum-mix clips onto a silent 48 kHz timeline; returns interleaved i16 samples.
pub fn build_track(clips: &[DubClip]) -> anyhow::Result<Vec<i16>> {
    let mut staged: Vec<(usize, Vec<i16>)> = Vec::new();
    let mut end = 0usize;
    for c in clips {
        let mut r = hound::WavReader::open(&c.wav_path)
            .with_context(|| format!("cannot read clip {}", c.wav_path.display()))?;
        let spec = r.spec();
        anyhow::ensure!(
            spec.channels == 1 && spec.sample_rate == 48000 && spec.bits_per_sample == 16,
            "clip {} not 48kHz mono s16",
            c.wav_path.display()
        );
        let samples: Vec<i16> = r.samples::<i16>().collect::<Result<_, _>>()?;
        let start = c.start_ms as usize * 48;
        end = end.max(start + samples.len());
        staged.push((start, samples));
    }
    let mut acc = vec![0i32; end];
    for (start, samples) in staged {
        for (j, s) in samples.iter().enumerate() {
            acc[start + j] += *s as i32;
        }
    }
    Ok(acc
        .iter()
        .map(|&v| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16)
        .collect())
}

pub fn write_track_wav(samples: &[i16], out: &Path) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(out, spec)
        .with_context(|| format!("cannot create {}", out.display()))?;
    for s in samples {
        w.write_sample(*s)?;
    }
    w.finalize()?;
    Ok(())
}

/// Full dub stage: probe → plan → synthesize+fit per segment → track → dubbed.wav.
/// Skips failed segments; errors if >50% fail or `is_cancelled` between segments.
/// Returns (wav_path, has_audio): callers reuse has_audio for mux dispatch.
pub async fn render_dub_track<P, C>(
    engine: &dyn TtsEngine,
    segs: &[SubtitleSegment],
    ffmpeg: &Path,
    work_dir: &Path,
    video_path: &Path,
    mut on_progress: P,
    mut is_cancelled: C,
) -> anyhow::Result<(PathBuf, bool)>
where
    P: FnMut(usize, usize) + Send,
    C: FnMut() -> bool + Send,
{
    let (video_end_ms, has_audio) = probe_media(ffmpeg, video_path);
    let plans = plan_segments(segs, video_end_ms);
    anyhow::ensure!(
        !plans.is_empty(),
        "no synthesizable subtitle text for dubbing"
    );
    let total = plans.len();
    let mut clips = Vec::with_capacity(total);
    let mut failures = 0usize;

    for (idx, plan) in plans.into_iter().enumerate() {
        if is_cancelled() {
            anyhow::bail!("dubbing cancelled");
        }
        on_progress(idx + 1, total);
        let raw_path = work_dir.join(format!("dub_raw_{idx}.bin"));
        let clip_path = work_dir.join(format!("dub_clip_{idx}.wav"));
        let fitted = async {
            let bytes = engine.synthesize(&plan.text).await?;
            tokio::fs::write(&raw_path, &bytes).await?;
            let dur_ms = match crate::media::wav_duration_ms(&raw_path) {
                Ok(d) => (raw_path.clone(), d),
                Err(_) => {
                    // Non-WAV payload (e.g. provider returned mp3): normalize first, then measure.
                    let probe = work_dir.join(format!("dub_probe_{idx}.wav"));
                    crate::media::run_ffmpeg(
                        crate::media::build_normalize_clip_command(ffmpeg, &raw_path, &probe, 1.0),
                        "dub probe normalize",
                    )?;
                    let d = crate::media::wav_duration_ms(&probe)?;
                    (probe, d)
                }
            };
            let (src, dur) = dur_ms;
            let tempo = decide_tempo(dur, plan.budget_ms);
            crate::media::run_ffmpeg(
                crate::media::build_normalize_clip_command(ffmpeg, &src, &clip_path, tempo),
                "dub clip normalize",
            )?;
            anyhow::Ok(DubClip {
                start_ms: plan.start_ms,
                wav_path: clip_path,
            })
        }
        .await;
        match fitted {
            Ok(c) => clips.push(c),
            Err(e) => {
                failures += 1;
                eprintln!(
                    "Warning: TTS failed for segment {}: {e:#}",
                    plan.seg_index + 1
                );
            }
        }
    }

    anyhow::ensure!(
        failures * 2 <= total,
        "dubbing failed for {failures}/{total} segments — aborting"
    );
    let samples = build_track(&clips)?;
    let out = work_dir.join("dubbed.wav");
    write_track_wav(&samples, &out)?;
    Ok((out, has_audio))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(id: usize, s: u64, e: u64) -> SubtitleSegment {
        SubtitleSegment::new(id, s, e, format!("line {id}"))
    }

    #[test]
    fn plans_budget_with_half_gap_cap() {
        let segs = vec![
            seg(1, 0, 1000),
            seg(2, 5000, 6000), // gap 4000 → 2000 cap
            seg(3, 6200, 7000), // gap 200 → 100
        ];
        let plans = plan_segments(&segs, Some(9000));
        assert_eq!(plans[0].slot_ms, 1000);
        assert_eq!(plans[0].budget_ms, 1000 + 2000); // cap
        assert_eq!(plans[1].budget_ms, 1000 + 100); // half of 200
        assert_eq!(plans[2].budget_ms, 800 + 1000); // last: (9000-7000)/2
    }

    #[test]
    fn last_gap_zero_without_duration() {
        let segs = vec![seg(1, 0, 1000)];
        assert_eq!(plan_segments(&segs, None)[0].budget_ms, 1000);
    }

    #[test]
    fn skips_empty_and_uses_original_when_untranslated() {
        let mut a = seg(1, 0, 1000);
        a.translated_text = Some("   ".into());
        let b = seg(2, 2000, 3000); // no translation → original_text
        let plans = plan_segments(&[a, b], None);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].text, "line 2");
    }

    #[test]
    fn tempo_decisions() {
        assert_eq!(decide_tempo(900, 1000), 1.0);
        assert_eq!(decide_tempo(1200, 1000), 1.2);
        assert_eq!(decide_tempo(9000, 1000), 1.5); // clamped
        assert_eq!(decide_tempo(100, 0), 1.5);
    }

    #[test]
    fn parses_ffmpeg_stderr() {
        let err = "Input #0, mov, from 'x.mp4':\n  Duration: 00:01:23.45, start: 0.0\n    Stream #0:1: Audio: aac";
        assert_eq!(parse_ffmpeg_duration_ms(err), Some(83_450));
        assert!(parse_has_audio(err));
        assert!(!parse_has_audio("Stream #0:0: Video: h264"));
    }

    fn write_sine_wav(path: &Path, ms: u64) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..(ms * 48) {
            w.write_sample(5000i16).unwrap();
        }
        w.finalize().unwrap();
    }

    #[test]
    fn track_places_clips_and_sums_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.wav");
        let b = dir.path().join("b.wav");
        write_sine_wav(&a, 1000);
        write_sine_wav(&b, 1000);
        let clips = vec![
            DubClip {
                start_ms: 0,
                wav_path: a,
            },
            DubClip {
                start_ms: 500,
                wav_path: b,
            }, // overlaps a by 500ms
        ];
        let track = build_track(&clips).unwrap();
        assert_eq!(track.len(), 1500 * 48);
        // overlap region (500-1000ms) has larger amplitude than single-clip region
        let solo = track[100 * 48].abs() as i32;
        let both = track[700 * 48].abs() as i32;
        assert!(both > solo || both == i16::MAX as i32);
    }

    #[test]
    fn wav_duration_reads_ms() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.wav");
        write_sine_wav(&p, 250);
        assert_eq!(crate::media::wav_duration_ms(&p).unwrap(), 250);
    }
}
