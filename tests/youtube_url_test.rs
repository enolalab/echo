use auto_subtitle::youtube::url::{parse_youtube_url, YoutubeUrlType};

#[test]
fn test_parse_single_video_standard() {
    let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    let res = parse_youtube_url(url).expect("should parse");
    assert_eq!(res, YoutubeUrlType::SingleVideo("dQw4w9WgXcQ".to_string()));
}

#[test]
fn test_parse_single_video_shortlink() {
    let url = "https://youtu.be/dQw4w9WgXcQ?t=42";
    let res = parse_youtube_url(url).expect("should parse");
    assert_eq!(res, YoutubeUrlType::SingleVideo("dQw4w9WgXcQ".to_string()));
}

#[test]
fn test_parse_shorts() {
    let url = "https://www.youtube.com/shorts/dQw4w9WgXcQ";
    let res = parse_youtube_url(url).expect("should parse");
    assert_eq!(res, YoutubeUrlType::SingleVideo("dQw4w9WgXcQ".to_string()));
}

#[test]
fn test_parse_pure_playlist() {
    let url = "https://www.youtube.com/playlist?list=PLrAXtmErZgOdP_8GztsuKi9nh508_u4tM";
    let res = parse_youtube_url(url).expect("should parse");
    assert_eq!(
        res,
        YoutubeUrlType::Playlist("PLrAXtmErZgOdP_8GztsuKi9nh508_u4tM".to_string())
    );
}

#[test]
fn test_parse_video_with_playlist() {
    let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLrAXtmErZgOdP_8GztsuKi9nh508_u4tM";
    let res = parse_youtube_url(url).expect("should parse");
    assert_eq!(
        res,
        YoutubeUrlType::VideoWithPlaylist {
            video_id: "dQw4w9WgXcQ".to_string(),
            playlist_id: "PLrAXtmErZgOdP_8GztsuKi9nh508_u4tM".to_string(),
        }
    );
}

#[test]
fn test_parse_invalid_url() {
    let url = "https://example.com/not-youtube";
    assert!(parse_youtube_url(url).is_err());
}
