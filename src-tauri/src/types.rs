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
}
