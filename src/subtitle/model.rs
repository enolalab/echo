use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleSegment {
    pub id: usize, // 1-based, sequential after sanitize
    pub start_ms: u64,
    pub end_ms: u64,
    pub original_text: String,
    pub translated_text: Option<String>,
}

impl SubtitleSegment {
    pub fn new(id: usize, start_ms: u64, end_ms: u64, original_text: impl Into<String>) -> Self {
        Self {
            id,
            start_ms,
            end_ms,
            original_text: original_text.into(),
            translated_text: None,
        }
    }

    pub fn format_timestamp_srt(ms: u64) -> String {
        let (h, m, s, milli) = (
            ms / 3_600_000,
            (ms / 60_000) % 60,
            (ms / 1000) % 60,
            ms % 1000,
        );
        format!("{h:02}:{m:02}:{s:02},{milli:03}")
    }

    pub fn format_timestamp_ass(ms: u64) -> String {
        let (h, m, s, cs) = (
            ms / 3_600_000,
            (ms / 60_000) % 60,
            (ms / 1000) % 60,
            (ms % 1000) / 10,
        );
        format!("{h}:{m:02}:{s:02}.{cs:02}")
    }
}

/// trim text; drop empty text; sort by start; clip overlap (prev.end = cur.start);
/// drop end <= start; renumber ids 1..=n.
pub fn sanitize_segments(mut segs: Vec<SubtitleSegment>) -> Vec<SubtitleSegment> {
    for s in &mut segs {
        s.original_text = s.original_text.trim().to_string();
        if let Some(t) = &mut s.translated_text {
            *t = t.trim().to_string();
        }
    }
    segs.retain(|s| !s.original_text.is_empty());
    segs.sort_by_key(|s| (s.start_ms, s.end_ms));
    for i in 1..segs.len() {
        let next_start = segs[i].start_ms;
        if segs[i - 1].end_ms > next_start {
            segs[i - 1].end_ms = next_start;
        }
    }
    segs.retain(|s| s.end_ms > s.start_ms);
    for (i, s) in segs.iter_mut().enumerate() {
        s.id = i + 1;
    }
    segs
}
