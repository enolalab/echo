use serde::{Deserialize, Serialize};

use crate::skills::Skill;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderKind {
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "anthropic_compatible")]
    AnthropicCompatible,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "ollama")]
    Ollama,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::OpenAiCompatible => "openai_compatible",
            Self::Anthropic => "anthropic",
            Self::AnthropicCompatible => "anthropic_compatible",
            Self::DeepSeek => "deepseek",
            Self::Ollama => "ollama",
        }
    }

    pub fn default_base_url(self) -> Option<&'static str> {
        match self {
            Self::OpenAi => Some("https://api.openai.com/v1"),
            Self::DeepSeek => Some("https://api.deepseek.com/v1"),
            Self::Anthropic => Some("https://api.anthropic.com"),
            Self::Ollama => Some("http://localhost:11434"),
            Self::OpenAiCompatible | Self::AnthropicCompatible => None,
        }
    }

    pub fn requires_base_url(self) -> bool {
        self.default_base_url().is_none()
    }

    pub fn requires_api_key(self) -> bool {
        !matches!(self, Self::Ollama)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    #[serde(default)]
    pub base_url: Option<String>,
    pub model: String,
}

impl ProviderProfile {
    pub fn effective_base_url(&self) -> Option<String> {
        self.base_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_end_matches('/').to_string())
            .or_else(|| self.kind.default_base_url().map(String::from))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct GeneralSettings {
    pub whisper_model: String,
    pub target_lang: String,
    pub bilingual: bool,
    pub style_preset: String,
    pub use_cloud_stt: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            whisper_model: "base".into(),
            target_lang: "vi".into(),
            bilingual: true,
            style_preset: "youtube_yellow".into(),
            use_cloud_stt: false,
        }
    }
}

fn default_version() -> u32 {
    SETTINGS_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<ProviderProfile>,
    #[serde(default)]
    pub default_profile_id: Option<String>,
    #[serde(default)]
    pub general: GeneralSettings,
    #[serde(default)]
    pub skills: Vec<Skill>,
    #[serde(default)]
    pub default_skill_id: Option<String>,
}

impl Settings {
    pub fn with_defaults() -> Self {
        let ollama = ProviderProfile {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Ollama local".into(),
            kind: ProviderKind::Ollama,
            base_url: None,
            model: "qwen2.5:7b".into(),
        };
        Self {
            version: SETTINGS_VERSION,
            default_profile_id: Some(ollama.id.clone()),
            profiles: vec![ollama],
            general: GeneralSettings::default(),
            skills: vec![],
            default_skill_id: None,
        }
    }

    pub fn validate_profile(&self, p: &ProviderProfile) -> Result<(), String> {
        let name = p.name.trim();
        if name.is_empty() {
            return Err("Tên profile không được để trống".into());
        }
        if self
            .profiles
            .iter()
            .any(|o| o.id != p.id && o.name.trim().to_lowercase() == name.to_lowercase())
        {
            return Err("Tên profile đã tồn tại".into());
        }
        if p.kind.requires_base_url() && p.effective_base_url().is_none() {
            return Err("Base URL bắt buộc với provider tương thích".into());
        }
        if p.model.trim().is_empty() {
            return Err("Model không được để trống".into());
        }
        Ok(())
    }

    pub fn find_profile(&self, name_or_id: &str) -> Option<&ProviderProfile> {
        let q = name_or_id.trim();
        self.profiles.iter().find(|p| p.id == q).or_else(|| {
            let lower = q.to_lowercase();
            self.profiles.iter().find(|p| p.name.trim().to_lowercase() == lower)
        })
    }

    pub fn default_profile(&self) -> Option<&ProviderProfile> {
        let id = self.default_profile_id.as_deref()?;
        self.profiles.iter().find(|p| p.id == id)
    }
}
