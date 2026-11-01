use serde::{Deserialize, Serialize};
use auto_subtitle::subtitle::SubtitleSegment;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhisperModelInfo {
    pub name: String,
    pub downloaded: bool,
    pub size_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemInfo {
    pub has_cuda: bool,
    pub ollama_running: bool,
    pub ollama_models: Vec<String>,
    pub whisper_models: Vec<WhisperModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineConfig {
    #[serde(default)]
    pub job_id: Option<String>,
    pub video_path: String,
    pub subtitle_path: Option<String>,
    pub whisper_model: String,
    pub use_cloud_stt: bool,
    pub stt_api_key: Option<String>,
    pub target_lang: String,
    pub skip_translate: bool,
    pub use_cloud_llm: bool,
    pub llm_model: String,
    pub llm_api_base: Option<String>,
    pub llm_api_key: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportMode {
    Hardsub,
    Softsub,
    SubsOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StylePreset {
    ClassicWhite,
    #[serde(rename = "youtube_yellow")]
    YouTubeYellow,
    NetflixModern,
    BilingualHighlight,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportRequest {
    pub video_path: String,
    pub output_path: String,
    pub mode: ExportMode,
    pub bilingual: bool,
    pub style_preset: StylePreset,
    pub segments: Vec<SubtitleSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PipelineProgressEvent {
    pub job_id: String,
    pub stage: String,
    pub message: String,
    pub percentage: f64,
    pub current_step: Option<usize>,
    pub total_steps: Option<usize>,
    pub elapsed_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_request_serialization() {
        let json = r#"{
            "video_path": "/path/video.mp4",
            "output_path": "/path/video_subtitled.mp4",
            "mode": "hardsub",
            "bilingual": true,
            "style_preset": "youtube_yellow",
            "segments": []
        }"#;
        let req: ExportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, ExportMode::Hardsub);
        assert_eq!(req.style_preset, StylePreset::YouTubeYellow);
        assert!(req.bilingual);
    }

    #[test]
    fn test_system_info_serialization() {
        let info = SystemInfo {
            has_cuda: false,
            ollama_running: true,
            ollama_models: vec!["qwen2.5:7b".to_string()],
            whisper_models: vec![WhisperModelInfo {
                name: "base".to_string(),
                downloaded: true,
                size_mb: 142,
            }],
        };
        let serialized = serde_json::to_string(&info).unwrap();
        assert!(serialized.contains("qwen2.5:7b"));
    }

    #[test]
    fn test_pipeline_progress_event_serialization() {
        let event = PipelineProgressEvent {
            job_id: "job-1".to_string(),
            stage: "translate".to_string(),
            message: "Đang dịch 15/45 đoạn...".to_string(),
            percentage: 72.5,
            current_step: Some(15),
            total_steps: Some(45),
            elapsed_secs: 28,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: PipelineProgressEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, event);
    }
}
