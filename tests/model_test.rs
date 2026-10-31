use auto_subtitle::config::AppConfig;
use auto_subtitle::subtitle::model::{sanitize_segments, SubtitleSegment as S};

#[test]
fn srt_timestamps() {
    assert_eq!(S::format_timestamp_srt(0), "00:00:00,000");
    assert_eq!(S::format_timestamp_srt(1234), "00:00:01,234");
    assert_eq!(S::format_timestamp_srt(3_661_050), "01:01:01,050");
}

#[test]
fn ass_timestamps() {
    assert_eq!(S::format_timestamp_ass(0), "0:00:00.00");
    assert_eq!(S::format_timestamp_ass(1234), "0:00:01.23");
    assert_eq!(S::format_timestamp_ass(3_661_050), "1:01:01.05");
}

#[test]
fn sanitize_sorts_clips_drops_and_renumbers() {
    let input = vec![
        S::new(9, 5000, 6000, "  third  "),
        S::new(1, 1000, 3000, "first"),
        S::new(2, 2500, 4000, "second overlaps first"),
        S::new(3, 7000, 7000, "zero duration"),
        S::new(4, 8000, 9000, "   "),
    ];
    let out = sanitize_segments(input);
    assert_eq!(out.len(), 3);
    assert_eq!((out[0].id, out[0].start_ms, out[0].end_ms), (1, 1000, 2500)); // clipped
    assert_eq!((out[1].id, out[1].start_ms, out[1].end_ms), (2, 2500, 4000));
    assert_eq!(out[2].original_text, "third"); // trimmed
    assert_eq!(out[2].id, 3);
}

#[test]
fn config_with_base_creates_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = AppConfig::with_base(tmp.path().join("c")).unwrap();
    assert!(cfg.models_dir.is_dir() && cfg.bin_dir.is_dir());
}
