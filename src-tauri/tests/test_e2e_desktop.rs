use auto_subtitle::subtitle::model::SubtitleSegment;
use auto_subtitle_desktop::commands::export_subtitles_core;
use auto_subtitle_desktop::types::{ExportMode, ExportRequest, StylePreset};

#[tokio::test]
async fn test_export_subs_only_e2e() {
    let temp_dir = tempfile::tempdir().unwrap();
    let out_sub = temp_dir.path().join("test_out.mp4");

    let req = ExportRequest {
        video_path: "jfk_video.mp4".to_string(),
        output_path: out_sub.to_str().unwrap().to_string(),
        mode: ExportMode::SubsOnly,
        bilingual: true,
        style_preset: StylePreset::YouTubeYellow,
        segments: vec![SubtitleSegment {
            id: 1,
            start_ms: 100,
            end_ms: 2500,
            original_text: "And so, my fellow Americans".to_string(),
            translated_text: Some("Và vì vậy, đồng bào của tôi".to_string()),
        }],
        dub: None,
        job_id: None,
        render_speed: "balanced".to_string(),
    };

    let res = export_subtitles_core(None, None, req).await;
    assert!(res.is_ok(), "export should succeed: {:?}", res.err());
    let srt_file = temp_dir.path().join("test_out.srt");
    let ass_file = temp_dir.path().join("test_out.ass");
    assert!(srt_file.is_file(), "SRT file must exist");
    assert!(ass_file.is_file(), "ASS file must exist");

    let srt_str = std::fs::read_to_string(&srt_file).unwrap();
    assert!(srt_str.contains("Và vì vậy, đồng bào của tôi"));
    assert!(srt_str.contains("And so, my fellow Americans"));
}
