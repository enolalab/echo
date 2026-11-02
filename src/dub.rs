use std::path::Path;

use crate::subtitle::model::SubtitleSegment;
use crate::tts::normalize_tts_text;

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
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"Duration: (\d+):(\d+):(\d+)(?:\.(\d+))?").unwrap()
    });
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
}
