use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtDlpStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeVideoItem {
    pub id: String,
    pub title: String,
    pub duration_seconds: Option<f64>,
    pub thumbnail_url: Option<String>,
    pub channel_name: Option<String>,
    pub webpage_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeInspectResult {
    pub is_playlist: bool,
    pub playlist_id: Option<String>,
    pub title: String,
    pub items: Vec<YoutubeVideoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeDownloadRequest {
    pub items: Vec<YoutubeVideoItem>,
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeProgressPayload {
    pub video_id: String,
    pub index: usize,
    pub total_items: usize,
    pub stage: String,
    pub percentage: f32,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub saved_path: Option<String>,
    pub error: Option<String>,
}
