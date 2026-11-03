#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use super::model::{YoutubeInspectResult, YoutubeVideoItem, YtDlpStatus};

#[derive(Debug, Clone)]
pub struct YtDlpManager {
    client: reqwest::Client,
}

impl Default for YtDlpManager {
    fn default() -> Self {
        Self::new()
    }
}

impl YtDlpManager {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    pub fn app_bin_path() -> PathBuf {
        let base_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("auto-subtitle")
            .join("bin");

        let bin_name = if cfg!(windows) {
            "yt-dlp.exe"
        } else {
            "yt-dlp"
        };
        base_dir.join(bin_name)
    }

    pub fn resolve_binary(&self) -> Option<PathBuf> {
        if let Ok(path) = which::which("yt-dlp") {
            return Some(path);
        }

        let app_path = Self::app_bin_path();
        if app_path.exists() {
            return Some(app_path);
        }

        None
    }

    /// Locate an ffmpeg binary for yt-dlp post-processing (merging streams).
    /// Order: system PATH → app's managed binary (downloaded by the pipeline
    /// into the cache bin dir) → the yt-dlp app bin dir as a last resort.
    pub fn resolve_ffmpeg(&self) -> Option<PathBuf> {
        if let Ok(path) = which::which("ffmpeg") {
            return Some(path);
        }
        let bin_name = if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        };
        [dirs::cache_dir(), dirs::data_local_dir()]
            .into_iter()
            .flatten()
            .map(|d| d.join("auto-subtitle").join("bin").join(bin_name))
            .find(|p| p.is_file())
    }

    pub async fn check_status(&self) -> YtDlpStatus {
        let bin_path = self.resolve_binary();
        match bin_path {
            Some(path) => {
                let output = Command::new(&path).arg("--version").output().await;

                match output {
                    Ok(out) if out.status.success() => {
                        let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        YtDlpStatus {
                            installed: true,
                            path: Some(path.to_string_lossy().to_string()),
                            version: Some(version),
                        }
                    }
                    _ => YtDlpStatus {
                        installed: true,
                        path: Some(path.to_string_lossy().to_string()),
                        version: None,
                    },
                }
            }
            None => YtDlpStatus {
                installed: false,
                path: None,
                version: None,
            },
        }
    }

    pub async fn ensure_binary(&self) -> Result<PathBuf> {
        if let Some(path) = self.resolve_binary() {
            return Ok(path);
        }

        let target_path = Self::app_bin_path();
        if let Some(parent) = target_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create app bin directory")?;
        }

        let download_url = if cfg!(target_os = "windows") {
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
        } else if cfg!(target_os = "macos") {
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
        } else {
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
        };

        let response = self
            .client
            .get(download_url)
            .send()
            .await
            .context("Failed to send request for yt-dlp binary")?;

        if !response.status().is_success() {
            bail!("Failed to download yt-dlp: HTTP {}", response.status());
        }

        let mut file = tokio::fs::File::create(&target_path)
            .await
            .context("Failed to create yt-dlp binary file")?;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error downloading chunk")?;
            file.write_all(&chunk)
                .await
                .context("Error writing chunk to file")?;
        }
        file.flush().await?;

        #[cfg(unix)]
        {
            let mut perms = tokio::fs::metadata(&target_path).await?.permissions();
            perms.set_mode(0o755);
            tokio::fs::set_permissions(&target_path, perms).await?;
        }

        Ok(target_path)
    }

    pub async fn inspect_url(&self, url: &str) -> Result<YoutubeInspectResult> {
        let bin_path = self
            .resolve_binary()
            .context("yt-dlp chưa được cài đặt. Vui lòng kiểm tra hoặc tải yt-dlp.")?;

        let output = Command::new(&bin_path)
            .arg("--dump-single-json")
            .arg("--flat-playlist")
            .arg("--no-warnings")
            .arg(url)
            .output()
            .await
            .context("Failed to execute yt-dlp inspect command")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            bail!("yt-dlp inspect error: {}", err.trim());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_inspect_json(&stdout)
    }

    pub async fn download_item<F>(
        &self,
        item: &YoutubeVideoItem,
        output_dir: &Path,
        on_progress: F,
    ) -> Result<PathBuf>
    where
        F: Fn(f32, Option<String>, Option<String>) + Send + Sync + 'static,
    {
        let bin_path = self.resolve_binary().context("yt-dlp chưa được cài đặt")?;

        tokio::fs::create_dir_all(output_dir).await?;

        // Format and template
        let out_template = output_dir
            .join("%(title).150B [%(id)s].%(ext)s")
            .to_string_lossy()
            .to_string();

        // Prefer H.264 video + AAC audio so the merged MP4 plays in HTML5
        // <video> (WebKitGTK can't decode YouTube's AV1 f399-style streams).
        // Falls back to a progressive MP4 (avc1+aac built in, no merge needed)
        // and finally to any single-file best.
        let mut cmd = Command::new(&bin_path);
        cmd.arg("-f")
            .arg("bv*[vcodec^=avc1]+ba[acodec^=mp4a]/b[ext=mp4]/b")
            .arg("--merge-output-format")
            .arg("mp4")
            .arg("-o")
            .arg(&out_template)
            .arg("--newline")
            .arg("--progress-template")
            .arg("%(progress.downloaded_bytes)d %(progress.total_bytes)d %(progress.speed)f %(progress.eta)d");

        if let Some(ffmpeg) = self.resolve_ffmpeg() {
            cmd.arg("--ffmpeg-location").arg(&ffmpeg);
        }

        let mut child = cmd
            .arg(&item.webpage_url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("Failed to spawn yt-dlp download process")?;

        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let trimmed = line.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let (Ok(downloaded), Ok(total)) =
                        (parts[0].parse::<f64>(), parts[1].parse::<f64>())
                    {
                        if total > 0.0 {
                            let pct = ((downloaded / total) * 100.0) as f32;
                            let speed = parts.get(2).and_then(|s| s.parse::<f64>().ok()).map(|s| {
                                if s > 1_048_576.0 {
                                    format!("{:.2} MB/s", s / 1_048_576.0)
                                } else {
                                    format!("{:.1} KB/s", s / 1024.0)
                                }
                            });
                            let eta = parts
                                .get(3)
                                .and_then(|e| e.parse::<u64>().ok())
                                .map(|sec| format!("{:02}:{:02}", sec / 60, sec % 60));
                            on_progress(pct, speed, eta);
                        }
                    }
                }
            }
        }

        let status = child.wait().await?;
        if !status.success() {
            bail!("yt-dlp download failed with status {}", status);
        }

        // Look for the saved file containing the item.id in the output dir
        let mut entries = tokio::fs::read_dir(output_dir).await?;
        let pattern = format!("[{}].mp4", item.id);
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.contains(&pattern) {
                    return Ok(path);
                }
            }
        }

        // Fallback: any non-temp .mp4 containing item.id. Names like
        // "[id].f399.mp4" are per-format temp files (often video-only) left
        // behind when merging failed — never hand those to the pipeline.
        let id_tag = format!("[{}]", item.id);
        let temp_tag = format!("[{}].f", item.id);
        let mut entries = tokio::fs::read_dir(output_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.contains(&id_tag) && name.ends_with(".mp4") && !name.contains(&temp_tag) {
                    return Ok(path);
                }
            }
        }

        bail!(
            "Download succeeded but output file could not be located in {:?}",
            output_dir
        );
    }
}

pub fn parse_inspect_json(json_str: &str) -> Result<YoutubeInspectResult> {
    let val: Value = serde_json::from_str(json_str).context("Invalid JSON output from yt-dlp")?;

    let is_playlist = val.get("_type").and_then(|t| t.as_str()) == Some("playlist");

    if is_playlist {
        let playlist_id = val
            .get("id")
            .and_then(|i| i.as_str())
            .map(|s| s.to_string());
        let title = val
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("Danh sách phát YouTube")
            .to_string();

        let mut items = Vec::new();
        if let Some(entries) = val.get("entries").and_then(|e| e.as_array()) {
            for entry in entries {
                if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                    let entry_title = entry
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("Video không tên")
                        .to_string();

                    let duration_seconds = entry.get("duration").and_then(|d| d.as_f64());
                    let channel_name = entry
                        .get("uploader")
                        .or_else(|| entry.get("channel"))
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string());

                    let thumbnail_url = entry
                        .get("thumbnail")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| {
                            entry
                                .get("thumbnails")
                                .and_then(|ts| ts.as_array())
                                .and_then(|arr| {
                                    arr.first()
                                        .and_then(|t| t.get("url"))
                                        .and_then(|u| u.as_str())
                                        .map(|s| s.to_string())
                                })
                        });

                    let webpage_url = entry
                        .get("url")
                        .or_else(|| entry.get("webpage_url"))
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", id));

                    items.push(YoutubeVideoItem {
                        id: id.to_string(),
                        title: entry_title,
                        duration_seconds,
                        thumbnail_url,
                        channel_name,
                        webpage_url,
                    });
                }
            }
        }

        Ok(YoutubeInspectResult {
            is_playlist: true,
            playlist_id,
            title,
            items,
        })
    } else {
        let id = val
            .get("id")
            .and_then(|i| i.as_str())
            .context("Missing video id in inspect response")?
            .to_string();

        let title = val
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("Video không tên")
            .to_string();

        let duration_seconds = val.get("duration").and_then(|d| d.as_f64());
        let channel_name = val
            .get("uploader")
            .or_else(|| val.get("channel"))
            .and_then(|u| u.as_str())
            .map(|s| s.to_string());

        let thumbnail_url = val
            .get("thumbnail")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                val.get("thumbnails")
                    .and_then(|ts| ts.as_array())
                    .and_then(|arr| {
                        arr.last()
                            .and_then(|t| t.get("url"))
                            .and_then(|u| u.as_str())
                            .map(|s| s.to_string())
                    })
            });

        let webpage_url = val
            .get("webpage_url")
            .or_else(|| val.get("url"))
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", id));

        let item = YoutubeVideoItem {
            id,
            title: title.clone(),
            duration_seconds,
            thumbnail_url,
            channel_name,
            webpage_url,
        };

        Ok(YoutubeInspectResult {
            is_playlist: false,
            playlist_id: None,
            title,
            items: vec![item],
        })
    }
}
