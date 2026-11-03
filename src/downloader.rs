use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt;

use crate::config::AppConfig;
use crate::tts::{VoiceInfo, HF_VOICES_BASE};

pub const WHISPER_MODELS: &[&str] = &[
    "tiny",
    "tiny.en",
    "base",
    "base.en",
    "small",
    "small.en",
    "medium",
    "medium.en",
    "large-v3",
    "large-v3-turbo",
];

const DEFAULT_MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
// BtbN build: unlike johnvansickle's minimal static build this ships the
// NVENC encoders (h264/hevc/av1_nvenc) + cuda hwaccel, so the GPU render path
// actually works. Pin the n8.1 branch (not master): master requires newer
// nvenc APIs than many installed drivers provide.
const DEFAULT_FFMPEG_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-n8.1-latest-linux64-gpl-8.1.tar.xz";

#[derive(Debug, Clone)]
pub struct DependencyManager {
    config: AppConfig,
    client: reqwest::Client,
    model_base_url: String,
    ffmpeg_url: String,
}

impl DependencyManager {
    pub fn new(config: AppConfig) -> Self {
        Self::with_urls(
            config,
            DEFAULT_MODEL_BASE_URL.to_string(),
            DEFAULT_FFMPEG_URL.to_string(),
        )
    }

    pub fn with_urls(config: AppConfig, model_base_url: String, ffmpeg_url: String) -> Self {
        let client = reqwest::Client::builder()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            config,
            client,
            model_base_url,
            ffmpeg_url,
        }
    }

    pub fn model_path(&self, name: &str) -> Result<PathBuf> {
        if !WHISPER_MODELS.contains(&name) {
            anyhow::bail!("unknown Whisper model: '{name}'");
        }
        Ok(self.config.models_dir.join(format!("ggml-{name}.bin")))
    }

    pub async fn ensure_whisper_model(&self, name: &str) -> Result<PathBuf> {
        let path = self.model_path(name)?;
        if path.is_file() {
            let metadata = fs::metadata(&path)
                .with_context(|| format!("failed to read metadata for {}", path.display()))?;
            if metadata.len() > 0 {
                return Ok(path);
            }
        }

        let base = self.model_base_url.trim_end_matches('/');
        let url = format!("{base}/ggml-{name}.bin");
        download_to(&self.client, &url, &path).await?;
        Ok(path)
    }

    pub async fn find_or_download_ffmpeg(&self) -> Result<PathBuf> {
        if let Ok(system_ffmpeg) = which::which("ffmpeg") {
            return Ok(system_ffmpeg);
        }

        let bin_ffmpeg = self.config.bin_dir.join("ffmpeg");
        if bin_ffmpeg.is_file() {
            return Ok(bin_ffmpeg);
        }

        if !(cfg!(target_os = "linux") && std::env::consts::ARCH == "x86_64") {
            anyhow::bail!(
                "Automatic FFmpeg download is only supported on Linux x86_64. \
                 Please install FFmpeg manually using your system package manager and ensure it is in your PATH."
            );
        }

        fs::create_dir_all(&self.config.bin_dir).with_context(|| {
            format!(
                "failed to create directory {}",
                self.config.bin_dir.display()
            )
        })?;

        let temp_dir = tempfile::tempdir()
            .context("failed to create temporary directory for ffmpeg extraction")?;
        let archive_path = temp_dir.path().join("ffmpeg.tar.xz");

        download_to(&self.client, &self.ffmpeg_url, &archive_path).await?;

        let status = tokio::process::Command::new("tar")
            .arg("-xJf")
            .arg(&archive_path)
            .arg("-C")
            .arg(temp_dir.path())
            .status()
            .await
            .with_context(|| format!("failed to run tar -xJf on {}", archive_path.display()))?;

        if !status.success() {
            anyhow::bail!("tar extraction failed with status: {status}");
        }

        let mut found_ffmpeg: Option<PathBuf> = None;
        let direct = temp_dir.path().join("ffmpeg");
        if direct.is_file() {
            found_ffmpeg = Some(direct);
        } else {
            // Archive layouts: <dir>/ffmpeg (johnvansickle) or <dir>/bin/ffmpeg (BtbN).
            for entry in fs::read_dir(temp_dir.path()).with_context(|| {
                format!("failed to read directory {}", temp_dir.path().display())
            })? {
                let entry = entry.context("failed to read directory entry")?;
                let path = entry.path();
                if path.is_dir() {
                    for candidate in [path.join("ffmpeg"), path.join("bin").join("ffmpeg")] {
                        if candidate.is_file() {
                            found_ffmpeg = Some(candidate);
                            break;
                        }
                    }
                    if found_ffmpeg.is_some() {
                        break;
                    }
                }
            }
        }

        let extracted_binary =
            found_ffmpeg.context("could not find ffmpeg binary in extracted archive")?;

        if fs::rename(&extracted_binary, &bin_ffmpeg).is_err() {
            fs::copy(&extracted_binary, &bin_ffmpeg).with_context(|| {
                format!("failed to copy ffmpeg binary to {}", bin_ffmpeg.display())
            })?;
            let _ = fs::remove_file(&extracted_binary);
        }

        #[cfg(unix)]
        {
            fs::set_permissions(&bin_ffmpeg, fs::Permissions::from_mode(0o755))
                .with_context(|| format!("failed to chmod 0755 {}", bin_ffmpeg.display()))?;
        }

        let verify_output = tokio::process::Command::new(&bin_ffmpeg)
            .arg("-version")
            .output()
            .await
            .with_context(|| format!("failed to verify ffmpeg at {}", bin_ffmpeg.display()))?;

        if !verify_output.status.success() {
            let _ = fs::remove_file(&bin_ffmpeg);
            anyhow::bail!(
                "downloaded ffmpeg binary failed verification with exit status {}",
                verify_output.status
            );
        }

        Ok(bin_ffmpeg)
    }

    /// Piper bundle dir: <bin_dir>/piper/ containing binary + lib/ + espeak-ng-data/.
    pub fn piper_bundle_dir(&self) -> PathBuf {
        self.config.bin_dir.join("piper")
    }

    pub fn find_piper(&self) -> Option<PathBuf> {
        if let Ok(p) = which::which("piper") {
            return Some(p);
        }
        #[cfg(windows)]
        let exe = self.piper_bundle_dir().join("piper.exe");
        #[cfg(not(windows))]
        let exe = self.piper_bundle_dir().join("piper");
        exe.is_file().then_some(exe)
    }

    /// Auto-download supported where a tar.gz asset exists (Linux x86_64, macOS
    /// x64/aarch64). Windows ships a .zip — install piper manually so it's on PATH.
    /// Pinned to tag 2023.11.14-2 (rhasspy/piper redirects to OHF-Voice/piper1-gpl,
    /// whose latest releases are the Python package, not the C++ binary bundles).
    pub async fn find_or_download_piper(&self) -> Result<PathBuf> {
        if let Some(p) = self.find_piper() {
            return Ok(p);
        }
        const TAG: &str = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2";
        let url = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "x86_64") => format!("{TAG}/piper_linux_x86_64.tar.gz"),
            ("macos", "x86_64") => format!("{TAG}/piper_macos_x64.tar.gz"),
            ("macos", "aarch64") => format!("{TAG}/piper_macos_aarch64.tar.gz"),
            (os, arch) => anyhow::bail!(
                "Automatic Piper download is not supported on {os}/{arch} \
                 (Windows bundles a .zip). Install piper manually and ensure it is in your PATH."
            ),
        };

        let temp_dir = tempfile::tempdir().context("failed to create temp dir for piper")?;
        let archive = temp_dir.path().join("piper.tar.gz");
        download_to(&self.client, &url, &archive).await?;

        let status = tokio::process::Command::new("tar")
            .arg("-xzf")
            .arg(&archive)
            .arg("-C")
            .arg(temp_dir.path())
            .status()
            .await
            .context("failed to run tar -xzf on piper archive")?;
        if !status.success() {
            anyhow::bail!("tar extraction failed with status: {status}");
        }

        // Archive extracts to a `piper/` directory — preserve the whole bundle.
        let extracted = temp_dir.path().join("piper");
        anyhow::ensure!(
            extracted.is_dir(),
            "piper archive did not contain piper/ directory"
        );
        let dest = self.piper_bundle_dir();
        if dest.exists() {
            fs::remove_dir_all(&dest)
                .with_context(|| format!("failed to clear {}", dest.display()))?;
        }
        fs::create_dir_all(&self.config.bin_dir)?;
        if fs::rename(&extracted, &dest).is_err() {
            copy_dir_all(&extracted, &dest)?;
        }

        #[cfg(unix)]
        {
            let exe = dest.join("piper");
            fs::set_permissions(&exe, fs::Permissions::from_mode(0o755))
                .with_context(|| format!("failed to chmod 0755 {}", exe.display()))?;
        }
        let out = tokio::process::Command::new(dest.join("piper"))
            .arg("--version")
            .output()
            .await
            .context("failed to verify piper binary")?;
        if !out.status.success() {
            let _ = fs::remove_dir_all(&dest);
            anyhow::bail!("downloaded piper failed --version check");
        }
        Ok(dest.join("piper"))
    }

    /// Paths for voice files: <tts_dir>/<id>.onnx and <id>.onnx.json.
    pub fn voice_paths(&self, voice_id: &str) -> (PathBuf, PathBuf) {
        (
            self.config.tts_dir.join(format!("{voice_id}.onnx")),
            self.config.tts_dir.join(format!("{voice_id}.onnx.json")),
        )
    }

    pub async fn ensure_voice(&self, voice: &VoiceInfo) -> Result<(PathBuf, PathBuf)> {
        let (onnx, json) = self.voice_paths(voice.id);
        if !onnx.is_file() {
            let url = format!("{HF_VOICES_BASE}/{}/{id}.onnx", voice.hf_dir, id = voice.id);
            download_to(&self.client, &url, &onnx).await?;
        }
        if !json.is_file() {
            let url = format!(
                "{HF_VOICES_BASE}/{}/{id}.onnx.json",
                voice.hf_dir,
                id = voice.id
            );
            download_to(&self.client, &url, &json).await?;
        }
        Ok((onnx, json))
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

pub async fn download_to(client: &reqwest::Client, url: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!("failed to create parent directory {}", parent.display())
            })?;
        }
    }

    let file_name = dest
        .file_name()
        .context("destination path missing file name")?
        .to_string_lossy();
    let part_path = dest.with_file_name(format!("{file_name}.part"));

    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to send GET request to {url}"))?;
    let resp = resp
        .error_for_status()
        .with_context(|| format!("HTTP error response from {url}"))?;

    let content_length = resp.content_length();
    let pb = match content_length {
        Some(len) => {
            let pb = ProgressBar::new(len);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .unwrap_or_else(|_| ProgressStyle::default_bar())
                    .progress_chars("#>-"),
            );
            pb
        }
        None => {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{elapsed_precise}] {bytes}")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            pb
        }
    };

    let write_result: Result<()> = async {
        let mut file = tokio::fs::File::create(&part_path)
            .await
            .with_context(|| format!("failed to create temporary file {}", part_path.display()))?;

        let mut stream = resp.bytes_stream();
        while let Some(chunk_res) = stream.next().await {
            let chunk =
                chunk_res.with_context(|| format!("failed to read byte stream from {url}"))?;
            file.write_all(&chunk)
                .await
                .with_context(|| format!("failed to write data to {}", part_path.display()))?;
            pb.inc(chunk.len() as u64);
        }
        file.flush()
            .await
            .with_context(|| format!("failed to flush {}", part_path.display()))?;
        Ok(())
    }
    .await;

    if let Err(err) = write_result {
        let _ = tokio::fs::remove_file(&part_path).await;
        pb.finish_and_clear();
        return Err(err);
    }

    pb.finish_and_clear();

    if let Err(err) = tokio::fs::rename(&part_path, dest).await {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(err).with_context(|| {
            format!(
                "failed to rename {} to {}",
                part_path.display(),
                dest.display()
            )
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_paths_layout() {
        let cfg = AppConfig::with_base(std::env::temp_dir().join("as-test-dl")).unwrap();
        let dm = DependencyManager::new(cfg);
        let (o, j) = dm.voice_paths("vi_VN-vais1000-medium");
        assert!(o.ends_with("vi_VN-vais1000-medium.onnx"));
        assert!(j.ends_with("vi_VN-vais1000-medium.onnx.json"));
    }
}
