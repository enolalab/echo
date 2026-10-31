use auto_subtitle::subtitle::parser::*;
use std::path::Path;

#[test]
fn srt_multiline_and_crlf() {
    let srt = "1\r\n00:00:01,000 --> 00:00:03,500\r\nHello world!\r\n\r\n2\r\n00:00:04,100 --> 00:00:06,200\r\nLine one\r\nLine two\r\n";
    let s = parse_srt(srt).unwrap();
    assert_eq!(s.len(), 2);
    assert_eq!(
        (s[0].start_ms, s[0].end_ms, s[0].original_text.as_str()),
        (1000, 3500, "Hello world!")
    );
    assert_eq!(s[1].original_text, "Line one\nLine two");
}

#[test]
fn vtt_header_note_and_cue_settings() {
    let vtt = "\u{feff}WEBVTT\nKind: captions\n\nNOTE a comment\n\n00:01.500 --> 00:03.000 align:start position:10%\nWebVTT test\n";
    let s = parse_vtt(vtt).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].start_ms, s[0].end_ms), (1500, 3000));
    assert_eq!(s[0].original_text, "WebVTT test");
}

#[test]
fn ass_dialogue_with_commas_and_tags() {
    let ass = "[Script Info]\nTitle: x\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.50,0:00:04.00,Default,,0,0,0,,{\\b1}Hi, there\\Nsecond line\n";
    let s = parse_ass(ass).unwrap();
    assert_eq!((s[0].start_ms, s[0].end_ms), (1500, 4000));
    assert_eq!(s[0].original_text, "Hi, there\nsecond line");
}

#[test]
fn malformed_timestamp_is_error() {
    assert!(parse_srt("1\n00:00:xx,000 --> 00:00:02,000\nBad\n").is_err());
}

#[test]
fn unknown_extension_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("a.txt");
    std::fs::write(&p, "x").unwrap();
    assert!(parse_subtitle_file(&p).is_err());
}

#[test]
fn file_dispatch_by_extension() {
    let s = parse_subtitle_file(Path::new("tests/fixtures/sample.srt")).unwrap();
    assert!(!s.is_empty());
    assert_eq!(s[0].id, 1);

    let v = parse_subtitle_file(Path::new("tests/fixtures/sample.vtt")).unwrap();
    assert!(!v.is_empty());
    assert_eq!(v[0].id, 1);

    let a = parse_subtitle_file(Path::new("tests/fixtures/sample.ass")).unwrap();
    assert!(!a.is_empty());
    assert_eq!(a[0].id, 1);
}

#[test]
fn empty_subtitle_file_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("empty.srt");
    std::fs::write(&p, "\n\n").unwrap();
    assert!(parse_subtitle_file(&p).is_err());
}
