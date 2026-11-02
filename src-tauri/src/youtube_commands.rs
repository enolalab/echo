use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, State};
use tokio::sync::broadcast;

use auto_subtitle::youtube::manager::YtDlpManager;
use auto_subtitle::youtube::model::{
    YoutubeDownloadRequest, YoutubeInspectResult, YoutubeProgressPayload, YoutubeVideoItem,
    YtDlpStatus,
};

use crate::state::AppState;

const YOUTUBE_CANCEL_KEY: &str = "youtube-download";
pub const YOUTUBE_PROGRESS_EVENT: &str = "youtube-download-progress";

#[tauri::command]
pub async fn check_yt_dlp_status() -> Result<YtDlpStatus, String> {
    Ok(YtDlpManager::new().check_status().await)
}

#[tauri::command]
pub async fn ensure_yt_dlp() -> Result<YtDlpStatus, String> {
    let manager = YtDlpManager::new();
    manager
        .ensure_binary()
        .await
        .map_err(|e| format!("Không tải được yt-dlp: {e:#}"))?;
    Ok(manager.check_status().await)
}

#[tauri::command]
pub async fn inspect_youtube_url(url: String) -> Result<YoutubeInspectResult, String> {
    YtDlpManager::new()
        .inspect_url(&url)
        .await
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn cancel_youtube_download(state: State<'_, AppState>) -> Result<(), String> {
    let senders = state.cancel_senders.lock().await;
    if let Some(sender) = senders.get(YOUTUBE_CANCEL_KEY) {
        let _ = sender.send(());
    }
    Ok(())
}

fn default_download_dir() -> PathBuf {
    dirs::video_dir()
        .or_else(dirs::download_dir)
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("auto-subtitle")
        .join("downloads")
}

async fn find_existing_download(output_dir: &Path, video_id: &str) -> Option<PathBuf> {
    let pattern = format!("[{video_id}].mp4");
    let mut entries = tokio::fs::read_dir(output_dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.contains(&pattern) {
                return Some(path);
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn emit_youtube_progress(
    app: &AppHandle,
    item: &YoutubeVideoItem,
    index: usize,
    total_items: usize,
    stage: &str,
    percentage: f32,
    speed: Option<String>,
    eta: Option<String>,
    saved_path: Option<String>,
    error: Option<String>,
) {
    let _ = app.emit(
        YOUTUBE_PROGRESS_EVENT,
        YoutubeProgressPayload {
            video_id: item.id.clone(),
            index,
            total_items,
            stage: stage.to_string(),
            percentage,
            speed,
            eta,
            saved_path,
            error,
        },
    );
}

#[tauri::command]
pub async fn download_youtube_items(
    app: AppHandle,
    state: State<'_, AppState>,
    req: YoutubeDownloadRequest,
) -> Result<Vec<String>, String> {
    let output_dir = req
        .output_dir
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_download_dir);

    tokio::fs::create_dir_all(&output_dir)
        .await
        .map_err(|e| format!("Không tạo được thư mục tải: {e}"))?;

    let (cancel_tx, mut cancel_rx) = broadcast::channel(1);
    {
        let mut senders = state.cancel_senders.lock().await;
        senders.insert(YOUTUBE_CANCEL_KEY.to_string(), cancel_tx);
    }

    let manager = YtDlpManager::new();
    let total = req.items.len();
    let mut saved_paths: Vec<String> = Vec::new();

    for (idx, item) in req.items.iter().enumerate() {
        if let Some(existing) = find_existing_download(&output_dir, &item.id).await {
            let path_str = existing.to_string_lossy().to_string();
            emit_youtube_progress(
                &app,
                item,
                idx,
                total,
                "completed",
                100.0,
                None,
                None,
                Some(path_str.clone()),
                None,
            );
            saved_paths.push(path_str);
            continue;
        }

        let app_cb = app.clone();
        let item_cb = item.clone();
        let on_progress = move |pct: f32, speed: Option<String>, eta: Option<String>| {
            emit_youtube_progress(
                &app_cb, &item_cb, idx, total, "downloading", pct, speed, eta, None, None,
            );
        };

        let result = tokio::select! {
            res = manager.download_item(item, &output_dir, on_progress) => Some(res),
            _ = cancel_rx.recv() => None,
        };

        match result {
            Some(Ok(path)) => {
                let path_str = path.to_string_lossy().to_string();
                emit_youtube_progress(
                    &app,
                    item,
                    idx,
                    total,
                    "completed",
                    100.0,
                    None,
                    None,
                    Some(path_str.clone()),
                    None,
                );
                saved_paths.push(path_str);
            }
            Some(Err(e)) => {
                emit_youtube_progress(
                    &app,
                    item,
                    idx,
                    total,
                    "error",
                    0.0,
                    None,
                    None,
                    None,
                    Some(format!("{e:#}")),
                );
            }
            None => {
                emit_youtube_progress(
                    &app,
                    item,
                    idx,
                    total,
                    "error",
                    0.0,
                    None,
                    None,
                    None,
                    Some("Đã hủy tải.".to_string()),
                );
                break;
            }
        }
    }

    {
        let mut senders = state.cancel_senders.lock().await;
        senders.remove(YOUTUBE_CANCEL_KEY);
    }

    Ok(saved_paths)
}
