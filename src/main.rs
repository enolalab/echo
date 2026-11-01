use anyhow::Context;
use clap::Parser;

use auto_subtitle::cli::{Cli, OutputMode, SttEngine, TranslateEngine};
use auto_subtitle::config::AppConfig;
use auto_subtitle::downloader::DependencyManager;
use auto_subtitle::pipeline::{self, Deps};
use auto_subtitle::stt::whisper_api::ApiTranscriber;
use auto_subtitle::stt::whisper_local::LocalWhisperTranscriber;
use auto_subtitle::stt::Transcriber;
use auto_subtitle::llm::ollama::{ensure_ollama_model, OllamaChatClient};
use auto_subtitle::llm::openai::OpenAiChatClient;
use auto_subtitle::translate::{LlmTranslator, Translator};

async fn run_app() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load().context("failed to load application configuration")?;
    let dep_mgr = DependencyManager::new(config);

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
                let api_key = cli
                    .api_key
                    .clone()
                    .context("--api-key or OPENAI_API_KEY required for STT API")?;
                Some(Box::new(ApiTranscriber::new(
                    api_key,
                    cli.api_base_url.clone(),
                    cli.stt_api_model.clone(),
                    lang_opt,
                )))
            }
        }
    };

    let translator_box: Option<Box<dyn Translator>> = if cli.skip_translate {
        None
    } else {
        match cli.translate_engine {
            TranslateEngine::Ollama => {
                let client = OllamaChatClient::new(cli.ollama_url.clone(), cli.ollama_model.clone());
                ensure_ollama_model(&client, &cli.ollama_url, &cli.ollama_model).await?;
                Some(Box::new(LlmTranslator::new(Box::new(client), None)))
            }
            TranslateEngine::OpenaiCompat => {
                let api_key = cli
                    .api_key
                    .clone()
                    .context("--api-key or OPENAI_API_KEY required for translation")?;
                let client = OpenAiChatClient::new(
                    cli.api_base_url.clone(),
                    Some(api_key),
                    cli.api_model.clone(),
                );
                Some(Box::new(LlmTranslator::new(Box::new(client), None)))
            }
        }
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
