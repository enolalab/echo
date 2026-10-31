use crate::subtitle::model::SubtitleSegment;
use std::fmt::Write;

/// Render subtitle segments into SRT format.
///
/// Text selection rule:
/// - `translated_text = Some(t)` and `bilingual = false`: `t`
/// - `translated_text = Some(t)` and `bilingual = true`: original on top, `t` below
/// - `translated_text = None`: original text
pub fn render_srt(segments: &[SubtitleSegment], bilingual: bool) -> String {
    let mut out = String::new();
    for (i, seg) in segments.iter().enumerate() {
        let id = if seg.id > 0 { seg.id } else { i + 1 };
        let start = SubtitleSegment::format_timestamp_srt(seg.start_ms);
        let end = SubtitleSegment::format_timestamp_srt(seg.end_ms);

        let text = match (&seg.translated_text, bilingual) {
            (Some(trans), false) => trans.clone(),
            (Some(trans), true) => format!("{}\n{}", seg.original_text, trans),
            (None, _) => seg.original_text.clone(),
        };

        let _ = write!(out, "{id}\n{start} --> {end}\n{text}\n\n");
    }
    out
}
