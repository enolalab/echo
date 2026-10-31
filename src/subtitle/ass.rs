use crate::subtitle::model::SubtitleSegment;
use std::fmt::Write;

/// Escape text for ASS subtitles:
/// Replace `{` -> `(`, `}` -> `)`, `\` -> `/`, then newlines -> `\N`.
pub fn escape_ass_text(text: &str) -> String {
    text.replace('{', "(")
        .replace('}', ")")
        .replace('\\', "/")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\\N")
}

/// Render subtitle segments into ASS (Advanced SubStation Alpha v4.00+) format.
///
/// Layout:
/// - Resolution: 1920x1080 (PlayResX 1920 / PlayResY 1080)
/// - Style `Default`: Arial 64, white, outline 3, Alignment 2, MarginV 40
/// - Style `Original`: Arial 44, light grey, Alignment 2, MarginV 120
///
/// In bilingual mode (when translated text is present):
/// - `Original` layer 0
/// - `Default` layer 1
pub fn render_ass(segments: &[SubtitleSegment], bilingual: bool, title: &str) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "[Script Info]");
    let _ = writeln!(out, "Title: {title}");
    let _ = writeln!(out, "ScriptType: v4.00+");
    let _ = writeln!(out, "WrapStyle: 0");
    let _ = writeln!(out, "ScaledBorderAndShadow: yes");
    let _ = writeln!(out, "PlayResX: 1920");
    let _ = writeln!(out, "PlayResY: 1080");
    let _ = writeln!(out);

    let _ = writeln!(out, "[V4+ Styles]");
    let _ = writeln!(
        out,
        "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding"
    );
    let _ = writeln!(
        out,
        "Style: Default,Arial,64,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,3,0,2,10,10,40,1"
    );
    let _ = writeln!(
        out,
        "Style: Original,Arial,44,&H00DCDCDC,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,120,1"
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "[Events]");
    let _ = writeln!(
        out,
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text"
    );

    for seg in segments {
        let start = SubtitleSegment::format_timestamp_ass(seg.start_ms);
        let end = SubtitleSegment::format_timestamp_ass(seg.end_ms);

        if bilingual {
            if let Some(trans) = &seg.translated_text {
                let orig = escape_ass_text(&seg.original_text);
                let _ = writeln!(out, "Dialogue: 0,{start},{end},Original,,0,0,0,,{orig}");
                let trans_esc = escape_ass_text(trans);
                let _ = writeln!(out, "Dialogue: 1,{start},{end},Default,,0,0,0,,{trans_esc}");
            } else {
                let orig = escape_ass_text(&seg.original_text);
                let _ = writeln!(out, "Dialogue: 0,{start},{end},Default,,0,0,0,,{orig}");
            }
        } else {
            let text = seg.translated_text.as_deref().unwrap_or(&seg.original_text);
            let text_esc = escape_ass_text(text);
            let _ = writeln!(out, "Dialogue: 0,{start},{end},Default,,0,0,0,,{text_esc}");
        }
    }

    out
}
