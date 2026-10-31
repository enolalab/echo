use auto_subtitle::subtitle::{
    ass::{escape_ass_text, render_ass},
    model::SubtitleSegment,
    srt::render_srt,
};

fn seg() -> SubtitleSegment {
    let mut s = SubtitleSegment::new(1, 1500, 4000, "Hello");
    s.translated_text = Some("Xin chào".into());
    s
}

#[test]
fn srt_mono_and_bilingual() {
    let mono = render_srt(&[seg()], false);
    assert_eq!(mono, "1\n00:00:01,500 --> 00:00:04,000\nXin chào\n\n");
    let bi = render_srt(&[seg()], true);
    assert!(bi.contains("Hello\nXin chào"));
}

#[test]
fn srt_untranslated_falls_back_to_original() {
    let s = SubtitleSegment::new(1, 0, 1000, "Only original");
    assert!(render_srt(&[s], true).contains("Only original"));
}

#[test]
fn ass_structure_and_bilingual_lines() {
    let out = render_ass(&[seg()], true, "Video Test");
    assert!(out.contains("[Script Info]") && out.contains("Title: Video Test"));
    assert!(out.contains("Style: Default,") && out.contains("Style: Original,"));
    assert!(out.contains("Dialogue: 0,0:00:01.50,0:00:04.00,Original,,0,0,0,,Hello"));
    assert!(out.contains("Dialogue: 1,0:00:01.50,0:00:04.00,Default,,0,0,0,,Xin chào"));
}

#[test]
fn ass_escapes_override_tags_and_newlines() {
    assert_eq!(escape_ass_text("{\\b1}a\nb"), "(/b1)a\\Nb");
}

#[test]
fn ass_mono_with_translation() {
    let out = render_ass(&[seg()], false, "Mono Test");
    assert!(out.contains("Dialogue: 0,0:00:01.50,0:00:04.00,Default,,0,0,0,,Xin chào"));
    assert!(!out.contains("Original,,0,0,0,,Hello"));
}

#[test]
fn ass_untranslated_falls_back_to_original() {
    let s = SubtitleSegment::new(1, 0, 1000, "Only original");
    let out = render_ass(&[s], true, "Fallback Test");
    assert!(out.contains("Dialogue: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,Only original"));
    assert!(!out.contains(",Original,,"));
}

#[test]
fn empty_segments_rendering() {
    assert_eq!(render_srt(&[], false), "");
    assert_eq!(render_srt(&[], true), "");
    let ass = render_ass(&[], false, "Empty");
    assert!(ass.contains("[Script Info]") && ass.contains("Title: Empty"));
    assert!(!ass.contains("Dialogue:"));
}
