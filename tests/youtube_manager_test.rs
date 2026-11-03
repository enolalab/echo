use auto_subtitle::youtube::manager::parse_inspect_json;

#[test]
fn test_parse_single_video_json() {
    let json_data = r#"{
        "_type": "video",
        "id": "dQw4w9WgXcQ",
        "title": "Rick Astley - Never Gonna Give You Up",
        "duration": 213,
        "thumbnail": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
        "uploader": "RickAstleyVEVO",
        "webpage_url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    }"#;

    let res = parse_inspect_json(json_data).expect("parse json");
    assert!(!res.is_playlist);
    assert_eq!(res.title, "Rick Astley - Never Gonna Give You Up");
    assert_eq!(res.items.len(), 1);
    assert_eq!(res.items[0].id, "dQw4w9WgXcQ");
    assert_eq!(res.items[0].duration_seconds, Some(213.0));
    assert_eq!(
        res.items[0].channel_name,
        Some("RickAstleyVEVO".to_string())
    );
    assert_eq!(
        res.items[0].thumbnail_url,
        Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg".to_string())
    );
}

#[test]
fn test_parse_playlist_json() {
    let json_data = r#"{
        "_type": "playlist",
        "id": "PLtest123",
        "title": "Sample Playlist",
        "entries": [
            {
                "id": "v1",
                "title": "Video 1",
                "duration": 60,
                "thumbnails": [{"url": "https://example.com/1.jpg"}],
                "uploader": "Uploader 1",
                "url": "https://www.youtube.com/watch?v=v1"
            },
            {
                "id": "v2",
                "title": "Video 2",
                "duration": 120,
                "thumbnails": [{"url": "https://example.com/2.jpg"}],
                "uploader": "Uploader 2",
                "url": "https://www.youtube.com/watch?v=v2"
            }
        ]
    }"#;

    let res = parse_inspect_json(json_data).expect("parse playlist");
    assert!(res.is_playlist);
    assert_eq!(res.playlist_id, Some("PLtest123".to_string()));
    assert_eq!(res.title, "Sample Playlist");
    assert_eq!(res.items.len(), 2);
    assert_eq!(res.items[0].id, "v1");
    assert_eq!(res.items[1].id, "v2");
    assert_eq!(res.items[0].duration_seconds, Some(60.0));
    assert_eq!(res.items[1].duration_seconds, Some(120.0));
    assert_eq!(
        res.items[0].thumbnail_url,
        Some("https://example.com/1.jpg".to_string())
    );
}
