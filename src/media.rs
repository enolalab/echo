use anyhow::Context;
use std::path::Path;
use std::process::Command;

/// 16 kHz mono s16le WAV for local Whisper.
pub fn build_extract_wav_command(ffmpeg: &Path, video: &Path, out_wav: &Path) -> Command {
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(video)
        .arg("-vn")
        .arg("-ar")
        .arg("16000")
        .arg("-ac")
        .arg("1")
        .arg("-c:a")
        .arg("pcm_s16le")
        .arg(out_wav);
    cmd
}

/// 16 kHz mono 32 kbps MP3 for cloud STT (keeps uploads under 25 MB for ~100 min).
pub fn build_extract_mp3_command(ffmpeg: &Path, video: &Path, out_mp3: &Path) -> Command {
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(video)
        .arg("-vn")
        .arg("-ar")
        .arg("16000")
        .arg("-ac")
        .arg("1")
        .arg("-c:a")
        .arg("libmp3lame")
        .arg("-b:a")
        .arg("32k")
        .arg(out_mp3);
    cmd
}

/// Runs ffmpeg with current_dir = ass_path.parent() and `-vf ass=<file_name>`.
/// Err if ass file name is not [A-Za-z0-9._-]+ (we always use "subtitle.ass").
/// video/output are made absolute with std::path::absolute.
pub fn build_hardsub_command(
    ffmpeg: &Path,
    video: &Path,
    ass_path: &Path,
    output: &Path,
) -> anyhow::Result<Command> {
    let file_name = ass_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("ass path has no valid filename")?;

    if file_name.is_empty()
        || !file_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        anyhow::bail!(
            "ass filename contains invalid characters (allowed: [A-Za-z0-9._-]): {file_name}"
        );
    }

    let abs_video = std::path::absolute(video)
        .with_context(|| format!("failed to make video path absolute: {}", video.display()))?;
    let abs_output = std::path::absolute(output)
        .with_context(|| format!("failed to make output path absolute: {}", output.display()))?;

    let mut cmd = Command::new(ffmpeg);
    if let Some(parent) = ass_path.parent() {
        cmd.current_dir(parent);
    }
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(&abs_video)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a?")
        .arg("-vf")
        .arg(format!("ass={file_name}"))
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("medium")
        .arg("-crf")
        .arg("20")
        .arg("-c:a")
        .arg("copy")
        .arg(&abs_output);
    Ok(cmd)
}

/// Subtitle codec by output extension: mp4|m4v|mov -> mov_text, mkv -> srt, else Err.
pub fn build_softsub_command(
    ffmpeg: &Path,
    video: &Path,
    srt_path: &Path,
    output: &Path,
) -> anyhow::Result<Command> {
    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .context("output path has no extension")?;

    let codec = match ext.as_str() {
        "mp4" | "m4v" | "mov" => "mov_text",
        "mkv" => "srt",
        _ => anyhow::bail!("unsupported output container format: .{ext}"),
    };

    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(video)
        .arg("-i")
        .arg(srt_path)
        .arg("-map")
        .arg("0:v")
        .arg("-map")
        .arg("0:a?")
        .arg("-map")
        .arg("1:0")
        .arg("-c:v")
        .arg("copy")
        .arg("-c:a")
        .arg("copy")
        .arg("-c:s")
        .arg(codec)
        .arg(output);
    Ok(cmd)
}

/// Normalize any input to 48 kHz mono s16 WAV; applies atempo when tempo > 1.001.
pub fn build_normalize_clip_command(
    ffmpeg: &Path,
    input: &Path,
    out_wav: &Path,
    tempo: f64,
) -> Command {
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(input);
    if tempo > 1.001 {
        cmd.arg("-af")
            .arg(format!("atempo={tempo:.4},aresample=48000"));
    } else {
        cmd.arg("-af").arg("aresample=48000");
    }
    cmd.arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("48000")
        .arg("-c:a")
        .arg("pcm_s16le")
        .arg(out_wav);
    cmd
}

/// WAV duration via header (no decode). Err for non-WAV inputs.
pub fn wav_duration_ms(path: &Path) -> anyhow::Result<u64> {
    let r = hound::WavReader::open(path)
        .with_context(|| format!("cannot read wav {}", path.display()))?;
    let spec = r.spec();
    anyhow::ensure!(spec.sample_rate > 0, "wav has zero sample rate");
    Ok(r.duration() as u64 * 1000 / spec.sample_rate as u64)
}

/// Runs the command; on non-zero exit returns Err containing the last 20 stderr lines.
pub fn run_ffmpeg(mut cmd: Command, what: &str) -> anyhow::Result<()> {
    let output = cmd
        .output()
        .with_context(|| format!("failed to execute ffmpeg for {what}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = stderr.lines().collect();
        let tail_lines = if lines.len() > 20 {
            &lines[lines.len() - 20..]
        } else {
            &lines[..]
        };
        let tail = tail_lines.join("\n");
        anyhow::bail!(
            "ffmpeg failed for {what} (exit code: {}):\n{tail}",
            output
                .status
                .code()
                .map_or_else(|| "terminated by signal".to_string(), |c| c.to_string())
        );
    }
    Ok(())
}
