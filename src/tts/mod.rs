use serde::{Deserialize, Serialize};

pub mod api;
pub mod piper;

pub const HF_VOICES_BASE: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TtsEngineKind {
    #[default]
    Local,
    Api,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DubOptions {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub engine: TtsEngineKind,
    #[serde(default)]
    pub voice: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "default_orig_volume")]
    pub orig_volume: f32,
}

fn default_orig_volume() -> f32 {
    0.15
}

impl Default for DubOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            engine: TtsEngineKind::Local,
            voice: None,
            model: None,
            orig_volume: default_orig_volume(),
        }
    }
}

#[async_trait::async_trait]
pub trait TtsEngine: Send + Sync {
    /// Synthesize text → audio bytes (WAV expected; other codecs tolerated downstream).
    async fn synthesize(&self, text: &str) -> anyhow::Result<Vec<u8>>;
    fn name(&self) -> &'static str;
}

#[derive(Debug)]
pub struct VoiceInfo {
    pub id: &'static str,
    pub lang: &'static str,
    /// Directory inside rhasspy/piper-voices on HuggingFace.
    pub hf_dir: &'static str,
}

pub const VOICE_CATALOG: &[VoiceInfo] = &[
    VoiceInfo {
        id: "vi_VN-vais1000-medium",
        lang: "vi",
        hf_dir: "vi/vi_VN/vais1000/medium",
    },
    VoiceInfo {
        id: "vi_VN-25hours_single-low",
        lang: "vi",
        hf_dir: "vi/vi_VN/25hours_single/low",
    },
    VoiceInfo {
        id: "en_US-lessac-medium",
        lang: "en",
        hf_dir: "en/en_US/lessac/medium",
    },
];

pub fn find_voice(id: &str) -> Option<&'static VoiceInfo> {
    VOICE_CATALOG.iter().find(|v| v.id == id)
}

/// First catalog voice whose lang matches `target_lang` (case-insensitive).
pub fn default_voice_for_lang(target_lang: &str) -> Option<&'static VoiceInfo> {
    let lang = target_lang.trim().to_lowercase();
    VOICE_CATALOG.iter().find(|v| v.lang == lang)
}

/// Flatten newlines to spaces and strip ASS override tags ({...}) —
/// piper treats each stdin line as a separate utterance.
pub fn normalize_tts_text(text: &str) -> String {
    static TAG_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = TAG_RE.get_or_init(|| regex::Regex::new(r"\{[^}]*\}").unwrap());
    let no_tags = re.replace_all(text, " ");
    no_tags.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_ass_tags_and_newlines() {
        assert_eq!(
            normalize_tts_text("Xin {\\i1}chào{\\i0}\nthế giới"),
            "Xin chào thế giới"
        );
        assert_eq!(normalize_tts_text("  a  \n b \r\n c  "), "a b c");
        assert_eq!(normalize_tts_text("{\\pos(1,2)}"), "");
    }

    #[test]
    fn default_voice_matches_lang() {
        assert_eq!(
            default_voice_for_lang("vi").unwrap().id,
            "vi_VN-vais1000-medium"
        );
        assert_eq!(
            default_voice_for_lang("en").unwrap().id,
            "en_US-lessac-medium"
        );
        assert!(default_voice_for_lang("ja").is_none());
        assert_eq!(
            default_voice_for_lang("VI").unwrap().id,
            "vi_VN-vais1000-medium"
        );
    }

    #[test]
    fn dub_options_serde_roundtrip() {
        let d: DubOptions = serde_json::from_str(
            r#"{"enabled":true,"engine":"local","orig_volume":0.2}"#,
        )
        .unwrap();
        assert!(d.enabled);
        assert_eq!(d.engine, TtsEngineKind::Local);
        assert_eq!(d.orig_volume, 0.2);
    }
}
