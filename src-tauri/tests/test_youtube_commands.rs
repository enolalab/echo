use auto_subtitle::youtube::model::{YoutubeDownloadRequest, YoutubeVideoItem};

#[test]
fn test_download_request_serialization() {
    let req = YoutubeDownloadRequest {
        items: vec![YoutubeVideoItem {
            id: "test1".into(),
            title: "Test Video".into(),
            duration_seconds: Some(120.0),
            thumbnail_url: None,
            channel_name: Some("Test Channel".into()),
            webpage_url: "https://youtube.com/watch?v=test1".into(),
        }],
        output_dir: Some("/tmp/downloads".into()),
    };

    let serialized = serde_json::to_string(&req).expect("serialize");
    let deserialized: YoutubeDownloadRequest =
        serde_json::from_str(&serialized).expect("deserialize");
    assert_eq!(deserialized.items.len(), 1);
    assert_eq!(deserialized.items[0].id, "test1");
    assert_eq!(deserialized.output_dir, Some("/tmp/downloads".into()));
}
