use anyhow::Context;
use clap::Parser;

use auto_subtitle::cli::{
    choose_translator, Cli, OutputMode, SttEngine, TranslateEngine, TranslatorChoice,
    DEFAULT_API_MODEL, DEFAULT_OLLAMA_MODEL, DEFAULT_OLLAMA_URL,
};
use auto_subtitle::config::AppConfig;
use auto_subtitle::downloader::DependencyManager;
use auto_subtitle::llm::ollama::{ensure_ollama_model, OllamaChatClient};
use auto_subtitle::llm::openai::OpenAiChatClient;
use auto_subtitle::llm::{build_chat_client, ChatClient};
use auto_subtitle::pipeline::{self, Deps};
use auto_subtitle::settings::{ProviderKind, SettingsStore};
use auto_subtitle::stt::whisper_api::ApiTranscriber;
use auto_subtitle::stt::whisper_local::LocalWhisperTranscriber;
use auto_subtitle::stt::Transcriber;
use auto_subtitle::translate::{LlmTranslator, Translator};

fn print_profiles() -> anyhow::Result<()> {
    let store = SettingsStore::open_default()?;
    let s = store.settings();
    if s.profiles.is_empty() {
        println!("(chưa có profile nào)");
    }
    for p in &s.profiles {
        let default = if s.default_profile_id.as_deref() == Some(p.id.as_str()) { "default" } else { "" };
        println!("{} | {} | {} | {}", p.name, p.kind.as_str(), p.model, default);
    }
    Ok(())
}

async fn build_translator(
    cli: &Cli,
    store: Option<&SettingsStore>,
) -> anyhow::Result<Box<dyn Translator>> {
    match choose_translator(cli, store.map(|s| s.settings()))? {
        TranslatorChoice::Legacy(TranslateEngine::Ollama) => {
            let url = cli.ollama_url.clone().unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string());
            let model = cli.ollama_model.clone().unwrap_or_else(|| DEFAULT_OLLAMA_MODEL.to_string());
            let client = OllamaChatClient::new(url.clone(), model.clone());
            ensure_ollama_model(&client, &url, &model).await?;
            Ok(Box::new(LlmTranslator::new(Box::new(client), None)))
        }
        TranslatorChoice::Legacy(TranslateEngine::OpenaiCompat) => {
            let api_key = cli
                .api_key
                .clone()
                .context("--api-key or OPENAI_API_KEY required for translation")?;
            let model = cli.api_model.clone().unwrap_or_else(|| DEFAULT_API_MODEL.to_string());
            let client = OpenAiChatClient::new(cli.api_base_url_or_default(), Some(api_key), model);
            Ok(Box::new(LlmTranslator::new(Box::new(client), None)))
        }
        TranslatorChoice::Profile(profile) => {
            let key = match store {
                Some(s) => s.profile_key(&profile.id)?,
                None => None,
            };
            let client: Box<dyn ChatClient> =
                build_chat_client(&profile, key).map_err(anyhow::Error::new)?;
            if profile.kind == ProviderKind::Ollama {
                let base = profile.effective_base_url().unwrap_or_default();
                ensure_ollama_model(client.as_ref(), &base, &profile.model).await?;
            }
            Ok(Box::new(LlmTranslator::new(client, None)))
        }
    }
}

async fn run_app() -> anyhow::Result<()> {
    if std::env::args().skip(1).any(|a| a == "--list-profiles") {
        return print_profiles();
    }
    let cli = Cli::parse();
    let config = AppConfig::load().context("failed to load application configuration")?;
    let dep_mgr = DependencyManager::new(config);
    let store = match SettingsStore::open_default() {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("Cảnh báo: không đọc được cài đặt ({e:#}); dùng giá trị mặc định.");
            None
        }
    };

    let ffmpeg = if cli.mode == OutputMode::SubsOnly && cli.subtitle.is_some() {
        None
    } else {
        Some(
            dep_mgr
                .find_or_download_ffmpeg()
                .await
                .context("failed to find or download ffmpeg")?,
        )
    };

    let transcriber_box: Option<Box<dyn Transcriber>> = if cli.subtitle.is_some() {
        None
    } else {
        let lang_opt = if cli.source_lang == "auto" {
            None
        } else {
            Some(cli.source_lang.clone())
        };

        match cli.stt_engine {
            SttEngine::Local => {
                let model_path = dep_mgr
                    .ensure_whisper_model(&cli.whisper_model)
                    .await
                    .with_context(|| {
                        format!("failed to ensure whisper model '{}'", cli.whisper_model)
                    })?;
                Some(Box::new(LocalWhisperTranscriber::new(
                    model_path, lang_opt, true,
                )))
            }
            SttEngine::Api => {
                let stored = store.as_ref().and_then(|s| s.stt_key().ok().flatten());
                let api_key = cli
                    .api_key
                    .clone()
                    .or(stored)
                    .context("--api-key or OPENAI_API_KEY required for STT API")?;
                Some(Box::new(ApiTranscriber::new(
                    api_key,
                    cli.api_base_url_or_default(),
                    cli.stt_api_model.clone(),
                    lang_opt,
                )))
            }
        }
    };

    let translator_box: Option<Box<dyn Translator>> = if cli.skip_translate {
        None
    } else {
        Some(build_translator(&cli, store.as_ref()).await?)
    };

    let deps = Deps {
        ffmpeg,
        transcriber: transcriber_box.as_deref(),
        translator: translator_box.as_deref(),
    };

    let report = pipeline::run(&cli, deps).await?;

    println!("Processed {} subtitle segments.", report.segments);
    for output in report.outputs {
        println!("Output: {}", output.display());
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run_app().await {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}
