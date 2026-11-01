use auto_subtitle::subtitle::SubtitleSegment;

#[test]
fn test_subtitle_segment_types() {
    let seg = SubtitleSegment {
        id: 1,
        start_ms: 1000,
        end_ms: 4000,
        original_text: "Hello".to_string(),
        translated_text: Some("Xin chào".to_string()),
    };
    assert_eq!(seg.id, 1);
    assert_eq!(seg.start_ms, 1000);
    assert_eq!(seg.end_ms, 4000);
    assert_eq!(seg.original_text, "Hello");
    assert_eq!(seg.translated_text.as_deref(), Some("Xin chào"));
}
