use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::broadcast;

use auto_subtitle::config::AppConfig;
use auto_subtitle::downloader::{DependencyManager, WHISPER_MODELS};
use auto_subtitle::media::{self, run_ffmpeg};
use auto_subtitle::stt::whisper_local::LocalWhisperTranscriber;
use auto_subtitle::stt::Transcriber;
use auto_subtitle::subtitle::ass::escape_ass_text;
use auto_subtitle::subtitle::model::{sanitize_segments, SubtitleSegment};
use auto_subtitle::subtitle::srt::render_srt;
use auto_subtitle::llm::build_chat_client;
use auto_subtitle::settings::SettingsStore;
use auto_subtitle::stt::whisper_api::ApiTranscriber;
use auto_subtitle::translate::{LlmTranslator, Translator};

use crate::state::AppState;
use crate::types::{
    ExportMode, ExportProgress, ExportRequest, PipelineConfig, PipelineProgressEvent, PiperStatus,
    StylePreset, SystemInfo, WhisperModelInfo,
};

#[allow(clippy::too_many_arguments)]
fn emit_progress(
    app: &AppHandle,
    job_id: &str,
    start_time: std::time::Instant,
    stage: &str,
    message: &str,
    percentage: f64,
    current_step: Option<usize>,
    total_steps: Option<usize>,
) {
    let _ = app.emit(
        "pipeline-progress",
        PipelineProgressEvent {
            job_id: job_id.to_string(),
            stage: stage.to_string(),
            message: message.to_string(),
            percentage: percentage.clamp(0.0, 100.0),
            current_step,
            total_steps,
            elapsed_secs: start_time.elapsed().as_secs(),
        },
    );
    let _ = app.emit(
        "pipeline-stage",
        serde_json::json!({
            "job_id": job_id,
            "stage": stage,
            "message": message,
        }),
    );
}

fn get_app_config() -> AppConfig {
    AppConfig::load().unwrap_or_else(|_| {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("auto-subtitle");
        AppConfig::with_base(cache_dir).expect("valid cache dir")
    })
}

#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    let has_cuda = std::process::Command::new("which")
        .arg("nvcc")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let (ollama_running, ollama_models) = match client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => {
            #[derive(serde::Deserialize)]
            struct TagModel {
                name: String,
            }
            #[derive(serde::Deserialize)]
            struct TagsResponse {
                models: Option<Vec<TagModel>>,
            }
            if let Ok(tags) = res.json::<TagsResponse>().await {
                let models = tags
                    .models
                    .unwrap_or_default()
                    .into_iter()
                    .map(|m| m.name)
                    .collect();
                (true, models)
            } else {
                (true, Vec::new())
            }
        }
        _ => (false, Vec::new()),
    };

    let config = get_app_config();
    let dm = DependencyManager::new(config);
    let mut whisper_models = Vec::new();
    let sizes = [
        ("tiny", 75),
        ("tiny.en", 75),
        ("base", 142),
        ("base.en", 142),
        ("small", 466),
        ("small.en", 466),
        ("medium", 1460),
        ("medium.en", 1460),
        ("large-v3", 3100),
        ("large-v3-turbo", 1600),
    ];

    for &name in WHISPER_MODELS {
        let downloaded = if let Ok(p) = dm.model_path(name) {
            p.is_file() && std::fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false)
        } else {
            false
        };
        let size_mb = sizes
            .iter()
            .find(|(m, _)| *m == name)
            .map(|(_, s)| *s)
            .unwrap_or(150);

        whisper_models.push(WhisperModelInfo {
            name: name.to_string(),
            downloaded,
            size_mb,
        });
    }

    Ok(SystemInfo {
        has_cuda,
        ollama_running,
        ollama_models,
        whisper_models,
    })
}

#[tauri::command]
pub async fn download_whisper_model(
    app: AppHandle,
    model_name: String,
) -> Result<(), String> {
    let config = get_app_config();
    let dm = DependencyManager::new(config);
    let _ = app.emit("pipeline-log", format!("Bắt đầu tải model Whisper '{model_name}'..."));
    dm.ensure_whisper_model(&model_name)
        .await
        .map_err(|e| format!("Tải model thất bại: {e:#}"))?;
    let _ = app.emit("pipeline-log", format!("Tải model '{model_name}' hoàn tất!"));
    let _ = app.emit("download-complete", model_name);
    Ok(())
}

#[tauri::command]
pub async fn cancel_pipeline(
    state: State<'_, AppState>,
    job_id: Option<String>,
) -> Result<(), String> {
    let sender_guard = state.cancel_senders.lock().await;
    match job_id {
        Some(id) => {
            if let Some(sender) = sender_guard.get(&id) {
                let _ = sender.send(());
            }
        }
        None => {
            for sender in sender_guard.values() {
                let _ = sender.send(());
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn start_pipeline(
    app: AppHandle,
    state: State<'_, AppState>,
    config: PipelineConfig,
) -> Result<Vec<SubtitleSegment>, String> {
    let job_id = config
        .job_id
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let (cancel_tx, cancel_rx) = broadcast::channel(1);
    {
        let mut senders = state.cancel_senders.lock().await;
        senders.insert(job_id.clone(), cancel_tx);
    }

    let result = run_pipeline(app, state.settings.clone(), config, job_id.clone(), cancel_rx).await;

    {
        let mut senders = state.cancel_senders.lock().await;
        senders.remove(&job_id);
    }

    result
}

async fn run_pipeline(
    app: AppHandle,
    settings: std::sync::Arc<std::sync::Mutex<SettingsStore>>,
    config: PipelineConfig,
    job_id: String,
    mut cancel_rx: broadcast::Receiver<()>,
) -> Result<Vec<SubtitleSegment>, String> {

    let input_video = PathBuf::from(&config.video_path);
    if !input_video.is_file() {
        return Err(format!("Không tìm thấy file video: {}", input_video.display()));
    }

    let (translator, stt_key, skill_line): (Option<Box<dyn Translator>>, Option<String>, Option<String>) = {
        let store = settings.lock().map_err(|_| "Lỗi khóa cài đặt".to_string())?;
        let (translator, skill_line) = if config.skip_translate {
            (None, None)
        } else {
            let profile = store
                .resolve_profile(config.profile_id.as_deref())
                .map_err(|e| format!("{e:#}"))?
                .ok_or_else(|| "Chưa cấu hình provider dịch".to_string())?;
            let key = store.profile_key(&profile.id).map_err(|e| format!("{e:#}"))?;
            let client = build_chat_client(&profile, key).map_err(|e| e.user_message())?;
            let skill = store
                .resolve_skill(config.skill_id.as_deref())
                .map_err(|e| format!("{e:#}"))?;
            let line = crate::skill_commands::skill_log_line(skill.as_ref());
            (Some(Box::new(LlmTranslator::new(client, skill)) as Box<dyn Translator>), Some(line))
        };
        let stt_key = if config.use_cloud_stt {
            Some(
                store
                    .stt_key()
                    .map_err(|e| format!("{e:#}"))?
                    .ok_or_else(|| "Chưa nhập API key Whisper".to_string())?,
            )
        } else {
            None
        };
        (translator, stt_key, skill_line)
    };

    if let Some(line) = skill_line {
        let _ = app.emit("pipeline-log", format!("[{job_id}] {line}"));
    }

    let app_config = get_app_config();
    let dm = DependencyManager::new(app_config);
    let ffmpeg = dm
        .find_or_download_ffmpeg()
        .await
        .map_err(|e| format!("Không tìm thấy FFmpeg: {e:#}"))?;

    let start_time = std::time::Instant::now();

    // Phase 1: Extract Audio
    emit_progress(
            &app,
            &job_id,
            start_time,
        "extract_audio",
        "Đang trích xuất âm thanh từ video...",
        0.0,
        None,
        None,
    );
    let temp_dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let audio_path = if stt_key.is_some() {
        temp_dir.path().join("audio.mp3")
    } else {
        temp_dir.path().join("audio_16k.wav")
    };

    let extract_cmd = if stt_key.is_some() {
        media::build_extract_mp3_command(&ffmpeg, &input_video, &audio_path)
    } else {
        media::build_extract_wav_command(&ffmpeg, &input_video, &audio_path)
    };
    let extract_res = tokio::select! {
        res = tokio::task::spawn_blocking(move || run_ffmpeg(extract_cmd, "extract wav")) => Some(res),
        _ = cancel_rx.recv() => None,
    };
    match extract_res {
        Some(Ok(Ok(()))) => {},
        Some(Ok(Err(e))) => return Err(format!("Lỗi trích xuất âm thanh: {e:#}")),
        Some(Err(e)) => return Err(format!("Lỗi luồng: {e:#}")),
        None => return Err("Tiến trình đã bị người dùng hủy.".to_string()),
    }
    emit_progress(
            &app,
            &job_id,
            start_time,
        "extract_audio",
        "Trích xuất âm thanh hoàn tất.",
        10.0,
        None,
        None,
    );

    // Phase 2: Transcribe (STT)
    emit_progress(
            &app,
            &job_id,
            start_time,
        "stt",
        &format!("Đang tải và chuẩn bị model Whisper ({})...", config.whisper_model),
        12.0,
        None,
        None,
    );

    let stt_label = if stt_key.is_some() {
        "Whisper API".to_string()
    } else {
        format!("Whisper ({})", config.whisper_model)
    };

    let transcriber: Box<dyn Transcriber> = match stt_key {
        Some(key) => Box::new(ApiTranscriber::new(
            key,
            "https://api.openai.com/v1".to_string(),
            "whisper-1".to_string(),
            None,
        )),
        None => {
            let model_path = dm
                .ensure_whisper_model(&config.whisper_model)
                .await
                .map_err(|e| format!("Lỗi tải model Whisper: {e:#}"))?;
            Box::new(LocalWhisperTranscriber::new(model_path, None, config.prefer_gpu))
        }
    };

    emit_progress(
            &app,
            &job_id,
            start_time,
        "stt",
        &format!("Đang nhận dạng âm thanh với {stt_label}..."),
        18.0,
        None,
        None,
    );

    let app_stt = app.clone();
    let model_name = stt_label.clone();
    let job_id_stt = job_id.clone();

    // Spawn smooth progress ticker during transcription
    let (ticker_stop_tx, mut ticker_stop_rx) = tokio::sync::oneshot::channel::<()>();
    let ticker_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(800));
        let mut current_pct = 20.0;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if current_pct < 58.0 {
                        current_pct += 1.2;
                    }
                    emit_progress(
                        &app_stt,
                        &job_id_stt,
                        start_time,
                        "stt",
                        &format!("Đang nhận dạng âm thanh với {}...", model_name),
                        current_pct,
                        None,
                        None,
                    );
                }
                _ = &mut ticker_stop_rx => {
                    break;
                }
            }
        }
    });

    let transcribe_res = tokio::select! {
        res = transcriber.transcribe(&audio_path) => Some(res),
        _ = cancel_rx.recv() => None,
    };
    let _ = ticker_stop_tx.send(());
    let _ = ticker_handle.await;

    let mut segments = match transcribe_res {
        Some(Ok(segs)) => segs,
        Some(Err(e)) => return Err(format!("Lỗi nhận dạng Whisper: {e:#}")),
        None => return Err("Tiến trình đã bị người dùng hủy.".to_string()),
    };

    segments = sanitize_segments(segments);
    if segments.is_empty() {
        return Err("Không nhận dạng được lời thoại nào trong video.".to_string());
    }

    emit_progress(
            &app,
            &job_id,
            start_time,
        "stt",
        &format!("Nhận dạng hoàn tất: tìm thấy {} đoạn thoại.", segments.len()),
        60.0,
        Some(segments.len()),
        Some(segments.len()),
    );

    // Phase 3: Translate
    if !config.skip_translate {
        emit_progress(
            &app,
            &job_id,
            start_time,
            "translate",
            &format!("Bắt đầu dịch {} đoạn phụ đề sang {}...", segments.len(), config.target_lang),
            60.0,
            Some(0),
            Some(segments.len()),
        );

        let translator = translator.expect("translator resolved when translation enabled");

        let mut segments_to_translate = segments.clone();
        let target_lang = config.target_lang.clone();
        let app_translate = app.clone();
        let target_lang_cb = target_lang.clone();
        let job_id_translate = job_id.clone();

        let on_progress = move |done: usize, total: usize| {
            let pct = 60.0 + ((done as f64 / total as f64) * 38.0);
            emit_progress(
                &app_translate,
                &job_id_translate,
                start_time,
                "translate",
                &format!("Đang dịch {}/{} đoạn phụ đề sang {}...", done, total, target_lang_cb),
                pct,
                Some(done),
                Some(total),
            );
        };

        let translate_res = tokio::select! {
            res = auto_subtitle::pipeline::translate_all_with_progress(
                translator.as_ref(),
                &mut segments_to_translate,
                "auto",
                &target_lang,
                Some(on_progress),
            ) => Some(res),
            _ = cancel_rx.recv() => None,
        };
        match translate_res {
            Some(Ok(())) => {
                segments = segments_to_translate;
            }
            Some(Err(e)) => return Err(format!("Lỗi dịch phụ đề: {e:#}")),
            None => return Err("Tiến trình đã bị người dùng hủy.".to_string()),
        };
    }

    emit_progress(
            &app,
            &job_id,
            start_time,
        "ready",
        &format!("Hoàn tất! Đã tạo {} đoạn phụ đề.", segments.len()),
        100.0,
        Some(segments.len()),
        Some(segments.len()),
    );

    Ok(segments)
}

fn render_custom_ass(segments: &[SubtitleSegment], bilingual: bool, title: &str, preset: StylePreset) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(out, "[Script Info]");
    let _ = writeln!(out, "Title: {title}");
    let _ = writeln!(out, "ScriptType: v4.00+");
    let _ = writeln!(out, "WrapStyle: 0");
    let _ = writeln!(out, "ScaledBorderAndShadow: yes");
    let _ = writeln!(out, "PlayResX: 1920");
    let _ = writeln!(out, "PlayResY: 1080");
    let _ = writeln!(out);

    let default_style = match preset {
        StylePreset::YouTubeYellow => {
            "Style: Default,Arial Bold,64,&H0000DEFF,&H000000FF,&H00000000,&H00000000,1,0,0,0,100,100,0,0,1,3.5,0,2,10,10,40,1"
        }
        StylePreset::ClassicWhite => {
            "Style: Default,Arial,64,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,3,0,2,10,10,40,1"
        }
        StylePreset::NetflixModern => {
            "Style: Default,Arial,58,&H00F0F0F0,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,3,1,0,2,10,10,40,1"
        }
        StylePreset::BilingualHighlight => {
            "Style: Default,Arial Bold,62,&H00FFFF00,&H000000FF,&H00400000,&H00000000,1,0,0,0,100,100,0,0,1,3,0,2,10,10,40,1"
        }
    };

    let original_style = "Style: Original,Arial,44,&H00DCDCDC,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,120,1";

    let _ = writeln!(out, "[V4+ Styles]");
    let _ = writeln!(
        out,
        "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding"
    );
    let _ = writeln!(out, "{default_style}");
    let _ = writeln!(out, "{original_style}");
    let _ = writeln!(out);

    let _ = writeln!(out, "[Events]");
    let _ = writeln!(
        out,
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text"
    );

    for seg in segments {
        let start = SubtitleSegment::format_timestamp_ass(seg.start_ms);
        let end = SubtitleSegment::format_timestamp_ass(seg.end_ms);

        let orig = escape_ass_text(&seg.original_text);
        let trans = seg.translated_text.as_deref().map(escape_ass_text);

        if bilingual {
            if let Some(t) = trans {
                let _ = writeln!(out, "Dialogue: 0,{start},{end},Original,,0,0,0,,{orig}");
                let _ = writeln!(out, "Dialogue: 1,{start},{end},Default,,0,0,0,,{t}");
            } else {
                let _ = writeln!(out, "Dialogue: 0,{start},{end},Default,,0,0,0,,{orig}");
            }
        } else {
            let line = trans.unwrap_or(orig);
            let _ = writeln!(out, "Dialogue: 0,{start},{end},Default,,0,0,0,,{line}");
        }
    }
    out
}

fn piper_status(dm: &DependencyManager) -> PiperStatus {
    match dm.find_piper() {
        Some(p) => {
            let version = std::process::Command::new(&p)
                .arg("--version")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            PiperStatus {
                installed: true,
                path: Some(p.display().to_string()),
                version,
            }
        }
        None => PiperStatus {
            installed: false,
            path: None,
            version: None,
        },
    }
}

#[tauri::command]
pub async fn check_piper_status() -> Result<PiperStatus, String> {
    let dm = DependencyManager::new(get_app_config());
    Ok(piper_status(&dm))
}

#[tauri::command]
pub async fn ensure_piper(app: AppHandle, state: State<'_, AppState>) -> Result<PiperStatus, String> {
    let dm = DependencyManager::new(get_app_config());
    let _ = app.emit("pipeline-log", "Đang tải Piper TTS...");
    dm.find_or_download_piper()
        .await
        .map_err(|e| format!("Tải Piper thất bại: {e:#}"))?;
    // Default voice follows settings target_lang (same rule as CLI).
    let lang = {
        let store = state.settings.lock().map_err(|_| "Lỗi khóa cài đặt")?;
        store.settings().general.target_lang.clone()
    };
    if let Some(v) = auto_subtitle::tts::default_voice_for_lang(&lang) {
        dm.ensure_voice(v)
            .await
            .map_err(|e| format!("Tải voice thất bại: {e:#}"))?;
    }
    let _ = app.emit("pipeline-log", "Piper TTS sẵn sàng!");
    Ok(piper_status(&dm))
}

async fn build_export_tts(
    dm: &DependencyManager,
    dub: &auto_subtitle::tts::DubOptions,
    state: &AppState,
) -> Result<Box<dyn auto_subtitle::tts::TtsEngine>, String> {
    use auto_subtitle::tts::{api::ApiTts, piper::PiperTts, TtsEngineKind};
    match dub.engine {
        TtsEngineKind::Local => {
            let piper = dm
                .find_or_download_piper()
                .await
                .map_err(|e| format!("{e:#}"))?;
            let (model, cfg) = match dub.voice.as_deref() {
                Some(v) if auto_subtitle::tts::find_voice(v).is_some() => dm
                    .ensure_voice(auto_subtitle::tts::find_voice(v).unwrap())
                    .await
                    .map_err(|e| format!("{e:#}"))?,
                Some(v) => {
                    let p = PathBuf::from(v);
                    let c = p.with_extension("onnx.json");
                    if !p.is_file() || !c.is_file() {
                        return Err(format!("Voice không hợp lệ: {v}"));
                    }
                    (p, c)
                }
                None => {
                    let lang = {
                        let store = state.settings.lock().map_err(|_| "Lỗi khóa cài đặt")?;
                        store.settings().general.target_lang.clone()
                    };
                    let info = auto_subtitle::tts::default_voice_for_lang(&lang).ok_or(format!(
                        "Không có giọng Piper nào cho '{lang}' — chọn voice hoặc engine api"
                    ))?;
                    dm.ensure_voice(info).await.map_err(|e| format!("{e:#}"))?
                }
            };
            Ok(Box::new(PiperTts::new(piper, model, cfg)))
        }
        TtsEngineKind::Api => {
            let key = {
                let store = state.settings.lock().map_err(|_| "Lỗi khóa cài đặt")?;
                store.stt_key().ok().flatten()
            }
            .ok_or("Chưa nhập API key cho TTS".to_string())?;
            Ok(Box::new(ApiTts::new(
                "https://api.openai.com/v1".to_string(),
                key,
                dub.model.clone().unwrap_or_else(|| "tts-1".into()),
                dub.voice.clone().unwrap_or_else(|| "alloy".into()),
            )))
        }
    }
}

#[tauri::command]
pub async fn export_subtitles(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ExportRequest,
) -> Result<String, String> {
    export_subtitles_core(Some(app), Some(&state), request).await
}

/// Export body shared by the Tauri command and tests.
/// `app`/`state` may be None in tests — dub then errors only if enabled.
pub async fn export_subtitles_core(
    app: Option<AppHandle>,
    state: Option<&AppState>,
    request: ExportRequest,
) -> Result<String, String> {
    let app_config = get_app_config();
    let dm = DependencyManager::new(app_config);
    let ffmpeg = dm
        .find_or_download_ffmpeg()
        .await
        .map_err(|e| format!("Không tìm thấy FFmpeg: {e:#}"))?;

    let video_in = PathBuf::from(&request.video_path);
    let out_path = PathBuf::from(&request.output_path);

    if out_path == video_in {
        return Err("Đường dẫn file xuất không được trùng với file video gốc!".to_string());
    }

    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let temp_dir = tempfile::tempdir().map_err(|e| e.to_string())?;

    let dub_track: Option<(PathBuf, bool)> =
        if let Some(dub) = request.dub.as_ref().filter(|d| d.enabled) {
            let state = state.ok_or("Lồng tiếng yêu cầu app state")?;
            let job_key = request.job_id.clone().unwrap_or_else(|| "export".into());
            let mut cancel_rx = {
                let mut senders = state.cancel_senders.lock().await;
                let (tx, rx) = broadcast::channel(1);
                senders.insert(job_key.clone(), tx);
                rx
            };
            let engine = build_export_tts(&dm, dub, state).await?;
            let app_c = app.clone();
            let jid = request.job_id.clone();
            // `cancel_rx` is moved in; try_recv(&mut self) works via FnMut bound.
            let result = auto_subtitle::dub::render_dub_track(
                engine.as_ref(),
                &request.segments,
                &ffmpeg,
                temp_dir.path(),
                &video_in,
                move |idx, total| {
                    if let Some(a) = &app_c {
                        let _ = a.emit(
                            "export-progress",
                            ExportProgress {
                                job_id: jid.clone(),
                                stage: "dubbing".into(),
                                index: idx,
                                total,
                            },
                        );
                        let _ = a.emit(
                            "pipeline-log",
                            format!("Đang lồng tiếng {idx}/{total}..."),
                        );
                    }
                },
                move || cancel_rx.try_recv().is_ok(),
            )
            .await;
            // Always release the cancel slot for this export (success or error).
            {
                let mut senders = state.cancel_senders.lock().await;
                senders.remove(&job_key);
            }
            Some(result.map_err(|e| format!("Lỗi lồng tiếng: {e:#}"))?)
        } else {
            None
        };

    // Signal the render stage so the export dialog can advance its step UI.
    if request.job_id.is_some() {
        if let Some(a) = &app {
            let _ = a.emit(
                "export-progress",
                ExportProgress {
                    job_id: request.job_id.clone(),
                    stage: "rendering".into(),
                    index: 0,
                    total: 0,
                },
            );
        }
    }

    // GPU encode (NVENC) only applies to hardsub — softsub copies the video stream.
    let prefer_gpu = state
        .and_then(|s| {
            s.settings
                .lock()
                .map(|st| st.settings().general.prefer_gpu)
                .ok()
        })
        .unwrap_or(false);
    let enc = media::EncoderOpts {
        nvenc: prefer_gpu && media::nvenc_available(&ffmpeg),
        speed: media::RenderSpeed::parse(&request.render_speed),
    };

    match request.mode {
        ExportMode::SubsOnly => {
            let srt_content = render_srt(&request.segments, request.bilingual);
            let ass_content = render_custom_ass(&request.segments, request.bilingual, "Echo", request.style_preset);
            let srt_out = out_path.with_extension("srt");
            let ass_out = out_path.with_extension("ass");
            std::fs::write(&srt_out, srt_content).map_err(|e| e.to_string())?;
            std::fs::write(&ass_out, ass_content).map_err(|e| e.to_string())?;
            if let Some((ref dw, _)) = dub_track {
                let wav_out = out_path.with_extension("dub.wav");
                std::fs::copy(dw, &wav_out).map_err(|e| e.to_string())?;
                return Ok(format!(
                    "Đã xuất file phụ đề + giọng đọc: {}, {}, {}",
                    srt_out.display(),
                    ass_out.display(),
                    wav_out.display()
                ));
            }
            Ok(format!("Đã xuất file phụ đề: {} và {}", srt_out.display(), ass_out.display()))
        }
        ExportMode::Hardsub => {
            let ass_content = render_custom_ass(&request.segments, request.bilingual, "Echo", request.style_preset);
            let temp_ass = temp_dir.path().join("sub.ass");
            std::fs::write(&temp_ass, ass_content).map_err(|e| e.to_string())?;
            let cmd = if let Some((ref dw, has_audio)) = dub_track {
                media::build_hardsub_dub_command(
                    &ffmpeg,
                    &video_in,
                    &temp_ass,
                    dw,
                    &out_path,
                    request.dub.as_ref().map(|d| d.orig_volume).unwrap_or(0.15),
                    has_audio,
                    enc,
                )
                .map_err(|e| format!("Lỗi tạo lệnh Hardsub: {e:#}"))?
            } else {
                media::build_hardsub_command(&ffmpeg, &video_in, &temp_ass, &out_path, enc)
                    .map_err(|e| format!("Lỗi tạo lệnh Hardsub: {e:#}"))?
            };
            tokio::task::spawn_blocking(move || run_ffmpeg(cmd, "hardsub"))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| format!("Lỗi xuất Hardsub: {e:#}"))?;
            Ok(format!("Đã xuất video Hardsub thành công: {}", out_path.display()))
        }
        ExportMode::Softsub => {
            let srt_content = render_srt(&request.segments, request.bilingual);
            let temp_srt = temp_dir.path().join("sub.srt");
            std::fs::write(&temp_srt, srt_content).map_err(|e| e.to_string())?;
            let cmd = if let Some((ref dw, has_audio)) = dub_track {
                media::build_softsub_dub_command(
                    &ffmpeg,
                    &video_in,
                    &temp_srt,
                    dw,
                    &out_path,
                    request.dub.as_ref().map(|d| d.orig_volume).unwrap_or(0.15),
                    has_audio,
                )
                .map_err(|e| format!("Lỗi tạo lệnh Softsub: {e:#}"))?
            } else {
                media::build_softsub_command(&ffmpeg, &video_in, &temp_srt, &out_path)
                    .map_err(|e| format!("Lỗi tạo lệnh Softsub: {e:#}"))?
            };
            tokio::task::spawn_blocking(move || run_ffmpeg(cmd, "softsub"))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| format!("Lỗi xuất Softsub: {e:#}"))?;
            Ok(format!("Đã xuất video Softsub thành công: {}", out_path.display()))
        }
    }
}

#[tauri::command]
pub async fn open_in_file_manager(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    let dir = if p.is_dir() { p } else { p.parent().unwrap_or(p) };

    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(dir).spawn();

    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(dir).spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(dir).spawn();

    Ok(())
}

#[tauri::command]
pub async fn pick_video_file() -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter(
            "Video & Audio",
            &["mp4", "mkv", "avi", "mov", "flv", "wmv", "webm", "mp3", "wav", "m4a", "flac", "ogg"],
        )
        .set_title("Chọn tệp Video / Audio")
        .pick_file()
        .await;

    Ok(file.map(|f| f.path().to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn pick_video_files() -> Result<Vec<String>, String> {
    let files = rfd::AsyncFileDialog::new()
        .add_filter(
            "Video & Audio",
            &["mp4", "mkv", "avi", "mov", "flv", "wmv", "webm", "mp3", "wav", "m4a", "flac", "ogg"],
        )
        .set_title("Chọn các tệp Video / Audio")
        .pick_files()
        .await;

    Ok(files
        .map(|fs| {
            fs.iter()
                .map(|f| f.path().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default())
}

#[tauri::command]
pub async fn get_output_dir() -> Result<String, String> {
    let dir = dirs::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Echo Studio")
        .join("outputs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn pick_directory(title: Option<String>) -> Result<Option<String>, String> {
    let mut dialog = rfd::AsyncFileDialog::new();
    if let Some(t) = title {
        dialog = dialog.set_title(&t);
    }
    let dir = dialog.pick_folder().await;
    Ok(dir.map(|d| d.path().to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn pick_save_file(default_name: String, extension: String) -> Result<Option<String>, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_file_name(&default_name)
        .add_filter(&extension, &[&extension])
        .set_title("Chọn vị trí lưu tệp")
        .save_file()
        .await;

    Ok(file.map(|f| f.path().to_string_lossy().to_string()))
}
