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

/// Layout coordinates for burned-in ASS rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssLayout {
    pub play_res_x: u32,
    pub play_res_y: u32,
    /// MarginV of the main `Default` style.
    pub margin_v: u32,
    /// MarginV of the `Original` (bilingual upper line) style.
    pub orig_margin_v: u32,
    /// Fontsize of the `Default` style.
    pub font_main: u32,
    /// Fontsize of the `Original` style.
    pub font_orig: u32,
}

/// The classic 1920x1080 layout used for standalone .ass exports.
pub const DEFAULT_LAYOUT: AssLayout = AssLayout {
    play_res_x: 1920,
    play_res_y: 1080,
    margin_v: 40,
    orig_margin_v: 120,
    font_main: 64,
    font_orig: 44,
};

/// Black bar height below the video for `--pad-subs` when no px is given:
/// 20% of frame height (30% for bilingual, which needs a second line),
/// rounded down to an even number for yuv420.
pub fn auto_sub_bar_height(video_h: u32, bilingual: bool) -> u32 {
    let px = if bilingual {
        video_h * 3 / 10
    } else {
        video_h / 5
    };
    px & !1
}

/// Layout for "subs below video": PlayRes matches the padded frame, so the
/// bottom `pad` pixels are a dedicated subtitle band that never covers the
/// picture. Fonts scale with the video height (64/44 per 1080 px, same as the
/// classic layout) and margins center the text block inside the band.
pub fn sub_bar_layout(video_w: u32, video_h: u32, pad: u32, bilingual: bool) -> AssLayout {
    let font_main = (video_h * 64 / 1080).max(10);
    let font_orig = (video_h * 44 / 1080).max(9);
    // Worst-case block: two Default lines + (bilingual) one Original line.
    let block = if bilingual {
        font_main * 12 / 5 + font_orig * 6 / 5 + font_main
    } else {
        font_main * 12 / 5
    };
    let margin_v = (pad.saturating_sub(block) / 2).max(6);
    AssLayout {
        play_res_x: video_w,
        play_res_y: video_h + pad,
        margin_v,
        orig_margin_v: margin_v + font_main * 6 / 5 + font_orig / 4,
        font_main,
        font_orig,
    }
}

/// Render subtitle segments into ASS (Advanced SubStation Alpha v4.00+) format.
///
/// In bilingual mode (when translated text is present):
/// - `Original` layer 0
/// - `Default` layer 1
pub fn render_ass(segments: &[SubtitleSegment], bilingual: bool, title: &str) -> String {
    render_ass_with_layout(segments, bilingual, title, &DEFAULT_LAYOUT)
}

/// Same as [`render_ass`] with explicit PlayRes and vertical margins.
pub fn render_ass_with_layout(
    segments: &[SubtitleSegment],
    bilingual: bool,
    title: &str,
    layout: &AssLayout,
) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "[Script Info]");
    let _ = writeln!(out, "Title: {title}");
    let _ = writeln!(out, "ScriptType: v4.00+");
    let _ = writeln!(out, "WrapStyle: 0");
    let _ = writeln!(out, "ScaledBorderAndShadow: yes");
    let _ = writeln!(out, "PlayResX: {}", layout.play_res_x);
    let _ = writeln!(out, "PlayResY: {}", layout.play_res_y);
    let _ = writeln!(out);

    let _ = writeln!(out, "[V4+ Styles]");
    let _ = writeln!(
        out,
        "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding"
    );
    let _ = writeln!(
        out,
        "Style: Default,Arial,{},&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,3,0,2,10,10,{},1",
        layout.font_main,
        layout.margin_v
    );
    let _ = writeln!(
        out,
        "Style: Original,Arial,{},&H00DCDCDC,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,{},1",
        layout.font_orig,
        layout.orig_margin_v
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle::model::SubtitleSegment;

    fn seg() -> SubtitleSegment {
        SubtitleSegment {
            id: 1,
            start_ms: 0,
            end_ms: 1000,
            original_text: "hello".into(),
            translated_text: None,
        }
    }

    #[test]
    fn sub_bar_keeps_subs_inside_the_pad_band() {
        // 1080p source + auto bar: text block must end below y=1080.
        let (w, h) = (1920, 1080);
        let pad = auto_sub_bar_height(h, false);
        let l = sub_bar_layout(w, h, pad, false);
        assert_eq!(l.play_res_y, h + pad);
        // Worst-case 2-line block bottom = PlayResY - margin_v; its top must
        // stay at/below the pad top (y = video_h).
        let block_top = l.play_res_y - l.margin_v - font_block(&l, false);
        assert!(
            block_top >= h,
            "sub block {block_top} must not enter video area {h}"
        );
    }

    #[test]
    fn sub_bar_bilingual_band_is_taller() {
        assert!(auto_sub_bar_height(1080, true) > auto_sub_bar_height(1080, false));
        let l = sub_bar_layout(1920, 1080, auto_sub_bar_height(1080, true), true);
        assert!(
            l.orig_margin_v > l.margin_v,
            "original line sits above default"
        );
    }

    #[test]
    fn sub_bar_fonts_scale_with_video_height() {
        let l = sub_bar_layout(320, 240, 48, false);
        assert!(l.font_main < 64, "small video gets a smaller font");
        let l = sub_bar_layout(1920, 1080, 216, false);
        assert_eq!(l.font_main, 64);
    }

    /// Approx px height of the Default-style text block.
    fn font_block(l: &AssLayout, bilingual: bool) -> u32 {
        if bilingual {
            l.font_main * 12 / 5 + l.font_orig * 6 / 5 + l.font_main
        } else {
            l.font_main * 12 / 5
        }
    }

    #[test]
    fn padded_ass_uses_layout_res_and_margins() {
        let l = sub_bar_layout(320, 240, 48, false);
        let ass = render_ass_with_layout(&[seg()], false, "t", &l);
        assert!(ass.contains("PlayResY: 288"));
        assert!(ass.contains(&format!("10,10,{},1", l.margin_v)));
    }
}
