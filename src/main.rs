use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;

use auto_subtitle::cli::{
    choose_skill, choose_translator, Cli, OutputMode, SttEngine, TranslateEngine, TranslatorChoice,
    TtsEngine, DEFAULT_API_MODEL, DEFAULT_OLLAMA_MODEL, DEFAULT_OLLAMA_URL,
};
use auto_subtitle::config::AppConfig;
use auto_subtitle::downloader::DependencyManager;
use auto_subtitle::llm::ollama::{ensure_ollama_model, OllamaChatClient};
use auto_subtitle::llm::openai::OpenAiChatClient;
use auto_subtitle::llm::{build_chat_client, ChatClient};
use auto_subtitle::pipeline::{self, Deps};
use auto_subtitle::settings::{ProviderKind, SettingsStore};
use auto_subtitle::skills::Skill;
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
        let default = if s.default_profile_id.as_deref() == Some(p.id.as_str()) {
            "default"
        } else {
            ""
        };
        println!(
            "{} | {} | {} | {}",
            p.name,
            p.kind.as_str(),
            p.model,
            default
        );
    }
    Ok(())
}

fn print_skills() -> anyhow::Result<()> {
    let store = SettingsStore::open_default()?;
    let default = store.settings().default_skill_id.clone();
    for s in store.skills_all() {
        let mark = if default.as_deref() == Some(s.id.as_str()) {
            "default"
        } else {
            ""
        };
        println!(
            "{} | {} | {} terms | {}",
            s.name,
            s.id,
            s.glossary.len(),
            mark
        );
    }
    Ok(())
}

async fn build_translator(
    cli: &Cli,
    store: Option<&SettingsStore>,
    skill: Option<Skill>,
) -> anyhow::Result<Box<dyn Translator>> {
    match choose_translator(cli, store.map(|s| s.settings()))? {
        TranslatorChoice::Legacy(TranslateEngine::Ollama) => {
            let url = cli
                .ollama_url
                .clone()
                .unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string());
            let model = cli
                .ollama_model
                .clone()
                .unwrap_or_else(|| DEFAULT_OLLAMA_MODEL.to_string());
            let client = OllamaChatClient::new(url.clone(), model.clone());
            ensure_ollama_model(&client, &url, &model).await?;
            Ok(Box::new(LlmTranslator::new(
                Box::new(client),
                skill.clone(),
            )))
        }
        TranslatorChoice::Legacy(TranslateEngine::OpenaiCompat) => {
            let api_key = cli
                .api_key
                .clone()
                .context("--api-key or OPENAI_API_KEY required for translation")?;
            let model = cli
                .api_model
                .clone()
                .unwrap_or_else(|| DEFAULT_API_MODEL.to_string());
            let client = OpenAiChatClient::new(cli.api_base_url_or_default(), Some(api_key), model);
            Ok(Box::new(LlmTranslator::new(
                Box::new(client),
                skill.clone(),
            )))
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
            Ok(Box::new(LlmTranslator::new(client, skill)))
        }
    }
}

async fn build_tts_engine(
    cli: &Cli,
    dm: &DependencyManager,
    store: Option<&SettingsStore>,
) -> anyhow::Result<Box<dyn auto_subtitle::tts::TtsEngine>> {
    use auto_subtitle::tts::{api::ApiTts, piper::PiperTts};
    match cli.tts_engine {
        TtsEngine::Local => {
            let piper = dm.find_or_download_piper().await?;
            let (model, cfg) = resolve_local_voice(cli, dm).await?;
            Ok(Box::new(PiperTts::new(piper, model, cfg)))
        }
        TtsEngine::Api => {
            let stored = store.and_then(|s| s.stt_key().ok().flatten());
            let key = cli
                .api_key
                .clone()
                .or(stored)
                .context("--api-key or OPENAI_API_KEY required for --tts-engine api")?;
            let voice = cli.tts_voice.clone().unwrap_or_else(|| "alloy".to_string());
            Ok(Box::new(ApiTts::new(
                cli.api_base_url_or_default(),
                key,
                cli.tts_model.clone(),
                voice,
            )))
        }
    }
}

async fn resolve_local_voice(
    cli: &Cli,
    dm: &DependencyManager,
) -> anyhow::Result<(PathBuf, PathBuf)> {
    match cli.tts_voice.as_deref() {
        Some(v) => {
            if let Some(info) = auto_subtitle::tts::find_voice(v) {
                dm.ensure_voice(info).await
            } else {
                let p = PathBuf::from(v);
                let cfg = p.with_extension("onnx.json");
                anyhow::ensure!(p.is_file(), "voice model not found: {v}");
                anyhow::ensure!(cfg.is_file(), "voice config missing: {}", cfg.display());
                Ok((p, cfg))
            }
        }
        None => {
            let info = auto_subtitle::tts::default_voice_for_lang(&cli.target_lang).with_context(
                || {
                    format!(
                        "không có giọng Piper nào cho ngôn ngữ '{}'; truyền --tts-voice hoặc --tts-engine api",
                        cli.target_lang
                    )
                },
            )?;
            dm.ensure_voice(info).await
        }
    }
}

async fn run_app() -> anyhow::Result<()> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if raw_args.iter().any(|a| a == "--list-profiles") {
        return print_profiles();
    }
    if raw_args.iter().any(|a| a == "--list-skills") {
        return print_skills();
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

    let ffmpeg = if cli.mode == OutputMode::SubsOnly && cli.subtitle.is_some() && !cli.dub {
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
                    model_path,
                    lang_opt,
                    !cli.no_gpu,
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
        let skill = choose_skill(&cli, store.as_ref())?;
        if let Some(s) = &skill {
            eprintln!("Skill: {}", s.name);
        }
        Some(build_translator(&cli, store.as_ref(), skill).await?)
    };

    let tts_box: Option<Box<dyn auto_subtitle::tts::TtsEngine>> = if cli.dub {
        Some(build_tts_engine(&cli, &dep_mgr, store.as_ref()).await?)
    } else {
        None
    };

    let deps = Deps {
        ffmpeg,
        transcriber: transcriber_box.as_deref(),
        translator: translator_box.as_deref(),
        tts: tts_box.as_deref(),
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
    // Kill in-flight ffmpeg children on Ctrl+C/SIGTERM — the default signal
    // handling would exit the process and orphan them.
    tokio::spawn(async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = signal(SignalKind::terminate()).ok();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = async {
                    match term.as_mut() {
                        Some(t) => { t.recv().await; }
                        None => std::future::pending::<()>().await,
                    }
                } => {}
            }
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
        auto_subtitle::media::kill_all_children();
        std::process::exit(130);
    });
    if let Err(err) = run_app().await {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}
