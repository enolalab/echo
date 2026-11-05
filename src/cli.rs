use std::path::PathBuf;

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputMode {
    Hardsub,
    Softsub,
    SubsOnly,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SttEngine {
    Local,
    Api,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslateEngine {
    Ollama,
    OpenaiCompat,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TtsEngine {
    Local,
    Api,
}

#[derive(clap::Parser, Debug, Clone)]
#[command(
    name = "auto-subtitle",
    about = "Automated video subtitle generation and translation CLI"
)]
pub struct Cli {
    #[arg(short, long)]
    pub input: PathBuf,

    #[arg(short, long)]
    pub output: Option<PathBuf>,

    #[arg(long = "subtitle", visible_alias = "sub")]
    pub subtitle: Option<PathBuf>,

    #[arg(long)]
    pub skip_translate: bool,

    #[arg(short = 's', long, default_value = "auto")]
    pub source_lang: String,

    #[arg(short = 't', long, default_value = "vi")]
    pub target_lang: String,

    #[arg(short, long, value_enum, default_value_t = OutputMode::Hardsub)]
    pub mode: OutputMode,

    #[arg(short, long)]
    pub bilingual: bool,

    #[arg(long, value_enum, default_value_t = SttEngine::Local)]
    pub stt_engine: SttEngine,

    #[arg(long, default_value = "small")]
    pub whisper_model: String,

    #[arg(long, default_value = "whisper-1")]
    pub stt_api_model: String,

    #[arg(long, value_enum)]
    pub translate_engine: Option<TranslateEngine>,

    #[arg(long)]
    pub ollama_model: Option<String>,

    #[arg(long)]
    pub ollama_url: Option<String>,

    #[arg(long, env = "OPENAI_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    #[arg(long)]
    pub api_base_url: Option<String>,

    #[arg(long)]
    pub api_model: Option<String>,

    /// Provider profile name or id from settings.json
    #[arg(long)]
    pub profile: Option<String>,

    /// Print configured provider profiles and exit
    #[arg(long)]
    pub list_profiles: bool,

    /// Translation skill name or id, or "none"
    #[arg(long)]
    pub skill: Option<String>,

    /// Print available translation skills and exit
    #[arg(long)]
    pub list_skills: bool,

    #[arg(long)]
    pub keep_temp: bool,

    /// Force local Whisper to run on CPU even when CUDA is available
    #[arg(long)]
    pub no_gpu: bool,

    /// Render speed preset for video re-encode (hardsub)
    #[arg(long, value_parser = ["fast", "balanced", "quality"], default_value = "balanced")]
    pub render_speed: String,

    /// Video re-encode quality target CRF 0-51 (hardsub); lower = better quality, bigger file
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u8).range(0..=51))]
    pub crf: u8,

    /// Add a black bar below the video and render subtitles inside it (hardsub only,
    /// never covers the picture). Optional bar height in px; omit for auto (20% of height)
    #[arg(long, value_name = "PX", num_args = 0..=1, default_missing_value = "0")]
    pub pad_subs: Option<u32>,

    /// Max concurrent translation requests (API: 4-8; local Ollama depends on server parallelism)
    #[arg(long, default_value_t = 4)]
    pub translate_concurrency: usize,

    /// Dub translated subtitles into a synthesized voice track
    #[arg(long)]
    pub dub: bool,

    #[arg(long, value_enum, default_value_t = TtsEngine::Local)]
    pub tts_engine: TtsEngine,

    /// Voice: catalog id (e.g. vi_VN-vais1000-medium), api voice name, or .onnx path
    #[arg(long)]
    pub tts_voice: Option<String>,

    #[arg(long, default_value = "tts-1")]
    pub tts_model: String,

    /// Base URL for API TTS (default: https://api.openai.com/v1; or e.g. http://localhost:8000/v1 for local VieNeu-TTS)
    #[arg(long)]
    pub tts_base_url: Option<String>,

    /// Original audio volume under the dub (0.0-1.0)
    #[arg(long, default_value = "0.15")]
    pub dub_orig_volume: f32,
}

use crate::settings::{ProviderProfile, Settings};

pub const DEFAULT_OLLAMA_MODEL: &str = "qwen2.5:7b";
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
pub const DEFAULT_API_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_API_MODEL: &str = "gpt-4o-mini";

impl Cli {
    pub fn api_base_url_or_default(&self) -> String {
        self.api_base_url
            .clone()
            .unwrap_or_else(|| DEFAULT_API_BASE_URL.to_string())
    }
}

#[derive(Debug, Clone)]
pub enum TranslatorChoice {
    Legacy(TranslateEngine),
    Profile(ProviderProfile),
}

pub fn choose_translator(
    cli: &Cli,
    settings: Option<&Settings>,
) -> anyhow::Result<TranslatorChoice> {
    if let Some(engine) = cli.translate_engine {
        return Ok(TranslatorChoice::Legacy(engine));
    }
    match (cli.profile.as_deref(), settings) {
        (Some(q), Some(s)) => s
            .find_profile(q)
            .cloned()
            .map(TranslatorChoice::Profile)
            .ok_or_else(|| {
                let names: Vec<&str> = s.profiles.iter().map(|p| p.name.as_str()).collect();
                anyhow::anyhow!("Không tìm thấy profile '{q}'. Có: {}", names.join(", "))
            }),
        (Some(_), None) => {
            anyhow::bail!("Không đọc được settings.json nên không dùng được --profile")
        }
        (None, Some(s)) => Ok(s
            .default_profile()
            .cloned()
            .map(TranslatorChoice::Profile)
            .unwrap_or(TranslatorChoice::Legacy(TranslateEngine::Ollama))),
        (None, None) => Ok(TranslatorChoice::Legacy(TranslateEngine::Ollama)),
    }
}

use crate::settings::SettingsStore;
use crate::skills::{Skill, NO_SKILL};

pub fn choose_skill(cli: &Cli, store: Option<&SettingsStore>) -> anyhow::Result<Option<Skill>> {
    match (cli.skill.as_deref().map(str::trim), store) {
        (Some(q), _) if q == NO_SKILL => Ok(None),
        (Some(q), Some(s)) => s.resolve_skill(Some(q)),
        (Some(_), None) => {
            anyhow::bail!("Không đọc được settings.json nên không dùng được --skill")
        }
        (None, Some(s)) => s.resolve_skill(None),
        (None, None) => Ok(None),
    }
}
