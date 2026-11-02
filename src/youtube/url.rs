use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YoutubeUrlType {
    SingleVideo(String),
    Playlist(String),
    VideoWithPlaylist {
        video_id: String,
        playlist_id: String,
    },
}

pub fn parse_youtube_url(url: &str) -> Result<YoutubeUrlType, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("URL không được để trống".into());
    }

    if !trimmed.contains("youtube.com") && !trimmed.contains("youtu.be") {
        return Err("Không nhận diện được link YouTube hợp lệ".into());
    }

    let re_v = Regex::new(
        r#"(?:v=|\/v\/|\/embed\/|\/shorts\/|^https?:\/\/youtu\.be\/|(?:\?|&)v=)([a-zA-Z0-9_-]{11})"#,
    )
    .map_err(|e| e.to_string())?;

    let re_list = Regex::new(r#"(?:list=)([a-zA-Z0-9_-]+)"#).map_err(|e| e.to_string())?;

    let video_id = re_v.captures(trimmed).map(|c| c[1].to_string());
    let playlist_id = re_list.captures(trimmed).map(|c| c[1].to_string());

    match (video_id, playlist_id) {
        (Some(v), Some(p)) => Ok(YoutubeUrlType::VideoWithPlaylist {
            video_id: v,
            playlist_id: p,
        }),
        (Some(v), None) => Ok(YoutubeUrlType::SingleVideo(v)),
        (None, Some(p)) => Ok(YoutubeUrlType::Playlist(p)),
        (None, None) => {
            Err("Không nhận diện được video ID hoặc playlist ID từ đường dẫn YouTube".into())
        }
    }
}
