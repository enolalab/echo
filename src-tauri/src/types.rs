use auto_subtitle::settings::{GeneralSettings, ProviderProfile, SecretsBackend};
use auto_subtitle::skills::Skill;
use auto_subtitle::subtitle::SubtitleSegment;
use auto_subtitle::tts::DubOptions;
use serde::{Deserialize, Serialize};

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

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineConfig {
    #[serde(default)]
    pub job_id: Option<String>,
    pub video_path: String,
    pub subtitle_path: Option<String>,
    pub whisper_model: String,
    pub use_cloud_stt: bool,
    pub target_lang: String,
    pub skip_translate: bool,
    #[serde(default = "default_true")]
    pub prefer_gpu: bool,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub skill_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExportRequest {
    pub video_path: String,
    pub output_path: String,
    pub mode: ExportMode,
    pub bilingual: bool,
    pub style_preset: StylePreset,
    pub segments: Vec<SubtitleSegment>,
    #[serde(default)]
    pub dub: Option<DubOptions>,
    #[serde(default)]
    pub job_id: Option<String>,
    /// "fast" | "balanced" | "quality" — hardsub re-encode only.
    #[serde(default = "default_render_speed")]
    pub render_speed: String,
    /// CRF 0-51 — hardsub re-encode only.
    #[serde(default = "default_crf")]
    pub crf: u8,
    /// Black subtitle-bar height in px added below the video (hardsub only):
    /// Some(0) = auto (20% of height), Some(px) = fixed, None = off.
    #[serde(default)]
    pub pad_subs: Option<u32>,
}

fn default_render_speed() -> String {
    "balanced".into()
}

fn default_crf() -> u8 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PiperStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProgress {
    pub job_id: Option<String>,
    pub stage: String,
    pub index: usize,
    pub total: usize,
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProfileView {
    #[serde(flatten)]
    pub profile: ProviderProfile,
    pub has_key: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SettingsView {
    pub profiles: Vec<ProfileView>,
    pub default_profile_id: Option<String>,
    pub general: GeneralSettings,
    pub presets: Vec<Skill>,
    pub skills: Vec<Skill>,
    pub default_skill_id: Option<String>,
    pub stt_has_key: bool,
    pub secrets_backend: SecretsBackend,
    pub load_warning: Option<String>,
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
