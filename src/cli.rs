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

    #[arg(long, value_enum, default_value_t = TranslateEngine::Ollama)]
    pub translate_engine: TranslateEngine,

    #[arg(long, default_value = "qwen2.5:7b")]
    pub ollama_model: String,

    #[arg(long, default_value = "http://localhost:11434")]
    pub ollama_url: String,

    #[arg(long, env = "OPENAI_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    #[arg(long, default_value = "https://api.openai.com/v1")]
    pub api_base_url: String,

    #[arg(long, default_value = "gpt-4o-mini")]
    pub api_model: String,

    #[arg(long)]
    pub keep_temp: bool,
}
