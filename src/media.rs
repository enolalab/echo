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
/// True when this ffmpeg binary ships the h264_nvenc hardware encoder.
/// Runs `ffmpeg -encoders` once per call (~10-30ms).
pub fn nvenc_available(ffmpeg: &Path) -> bool {
    std::process::Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-encoders")
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("h264_nvenc"))
        .unwrap_or(false)
}

/// Render speed/quality trade-off for video re-encode (hardsub only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderSpeed {
    Fast,
    #[default]
    Balanced,
    Quality,
}

impl RenderSpeed {
    /// Tolerant parser: anything unrecognized falls back to Balanced.
    pub fn parse(s: &str) -> Self {
        match s {
            "fast" => Self::Fast,
            "quality" | "high" => Self::Quality,
            _ => Self::Balanced,
        }
    }
}

/// Encoder selection for re-encode paths: device + speed preset + quality target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncoderOpts {
    pub nvenc: bool,
    pub speed: RenderSpeed,
    /// CRF/CQ quality target (0-51): lower = better quality, larger file.
    pub crf: u8,
}

/// Push `-hwaccel cuda` before an input arg (only for the NVENC path):
/// decode on GPU; ffmpeg auto-inserts hwdownload for CPU-side filters (ass).
fn push_hwaccel(cmd: &mut Command, enc: EncoderOpts) {
    if enc.nvenc {
        cmd.arg("-hwaccel").arg("cuda");
    }
}

fn push_video_encoder(cmd: &mut Command, enc: EncoderOpts) {
    if enc.nvenc {
        // NVENC: cq ~ x264 crf; p2/p4/p6 = fast/balanced/quality.
        let preset = match enc.speed {
            RenderSpeed::Fast => "p2",
            RenderSpeed::Balanced => "p4",
            RenderSpeed::Quality => "p6",
        };
        cmd.arg("-c:v")
            .arg("h264_nvenc")
            .arg("-preset")
            .arg(preset)
            .arg("-cq")
            .arg(enc.crf.to_string());
    } else {
        let preset = match enc.speed {
            RenderSpeed::Fast => "veryfast",
            RenderSpeed::Balanced => "medium",
            RenderSpeed::Quality => "slow",
        };
        cmd.arg("-c:v")
            .arg("libx264")
            .arg("-preset")
            .arg(preset)
            .arg("-crf")
            .arg(enc.crf.to_string());
    }
}

fn output_ext(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Parse WxH of the first `Video:` stream line in `ffmpeg -i` stderr.
pub fn parse_video_size(stderr: &str) -> Option<(u32, u32)> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"Stream[^\n]*Video:[^\n]*?(\d{2,5})x(\d{2,5})").unwrap()
    });
    let c = re.captures(stderr)?;
    Some((c[1].parse().ok()?, c[2].parse().ok()?))
}

/// `ffmpeg -i <video>` probe → (width, height) of the first video stream.
pub fn probe_video_size(ffmpeg: &Path, video: &Path) -> Option<(u32, u32)> {
    let out = std::process::Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(video)
        .output()
        .ok()?;
    parse_video_size(&String::from_utf8_lossy(&out.stderr))
}

/// Codec for a newly injected srt track by container: mp4-family needs mov_text.
fn new_sub_codec(output: &Path) -> anyhow::Result<&'static str> {
    let ext = output_ext(output);
    match ext.as_str() {
        "mp4" | "m4v" | "mov" => Ok("mov_text"),
        "mkv" => Ok("srt"),
        _ => anyhow::bail!("unsupported output container format: .{ext}"),
    }
}

/// Codec for passthrough of the input's own subtitle tracks: mp4-family only
/// accepts mov_text (transcodes srt/ass); elsewhere copy keeps the original codec.
fn passthrough_sub_codec(output: &Path) -> &'static str {
    match output_ext(output).as_str() {
        "mp4" | "m4v" | "mov" => "mov_text",
        _ => "copy",
    }
}

/// Map input 0's non-video streams through + global metadata/chapters.
/// The caller maps the (filtered or copied) video and audio itself.
fn push_passthrough_streams(cmd: &mut Command, output: &Path) {
    cmd.arg("-map")
        .arg("0:s?")
        .arg("-map")
        .arg("0:d?")
        .arg("-map")
        .arg("0:t?")
        .arg("-map_metadata")
        .arg("0")
        .arg("-map_chapters")
        .arg("0")
        .arg("-c:d")
        .arg("copy")
        .arg("-c:t")
        .arg("copy")
        .arg("-c:s")
        .arg(passthrough_sub_codec(output));
}

/// pad > 0 → extend the frame downward by `pad` black px and render the
/// (matching PlayRes) ASS into that band, so subtitles never cover the picture.
pub fn build_hardsub_command(
    ffmpeg: &Path,
    video: &Path,
    ass_path: &Path,
    output: &Path,
    pad: u32,
    enc: EncoderOpts,
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

    // Even heights only: yuv420 (libx264/nvenc) can't encode odd heights.
    let pad = pad & !1;
    let vgraph = if pad > 0 {
        format!("[0:v]pad=iw:ih+{pad}:0:0:black,ass={file_name}[v]")
    } else {
        format!("[0:v]ass={file_name}[v]")
    };

    let mut cmd = Command::new(ffmpeg);
    if let Some(parent) = ass_path.parent() {
        cmd.current_dir(parent);
    }
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y");
    push_hwaccel(&mut cmd, enc);
    cmd.arg("-i")
        .arg(&abs_video)
        .arg("-filter_complex")
        .arg(vgraph)
        .arg("-map")
        .arg("[v]")
        .arg("-map")
        .arg("0:a?")
        .arg("-c:a")
        .arg("copy");
    push_passthrough_streams(&mut cmd, &abs_output);
    push_video_encoder(&mut cmd, enc);
    cmd.arg(&abs_output);
    Ok(cmd)
}

/// hardsub + dub in one encode: single -filter_complex with ass burn + amix.
/// has_audio=false → burn subs, use dubbed track as the only audio.
#[allow(clippy::too_many_arguments)]
pub fn build_hardsub_dub_command(
    ffmpeg: &Path,
    video: &Path,
    ass_path: &Path,
    dub_wav: &Path,
    output: &Path,
    orig_volume: f32,
    has_audio: bool,
    pad: u32,
    enc: EncoderOpts,
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
    let abs_dub = std::path::absolute(dub_wav)
        .with_context(|| format!("failed to make dub wav absolute: {}", dub_wav.display()))?;
    let abs_output = std::path::absolute(output)
        .with_context(|| format!("failed to make output path absolute: {}", output.display()))?;

    let pad = pad & !1;
    let vgraph = if pad > 0 {
        format!("[0:v]pad=iw:ih+{pad}:0:0:black,ass={file_name}[v]")
    } else {
        format!("[0:v]ass={file_name}[v]")
    };
    let graph = if has_audio {
        format!(
            "{vgraph};[0:a]volume={orig_volume}[a0];[1:a]apad[a1];[a0][a1]amix=inputs=2:duration=longest:normalize=0[aout]"
        )
    } else {
        vgraph
    };

    let mut cmd = Command::new(ffmpeg);
    if let Some(parent) = ass_path.parent() {
        cmd.current_dir(parent);
    }
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y");
    // hwaccel applies only to the video input (first -i).
    push_hwaccel(&mut cmd, enc);
    cmd.arg("-i")
        .arg(&abs_video)
        .arg("-i")
        .arg(&abs_dub)
        .arg("-filter_complex")
        .arg(graph)
        .arg("-map")
        .arg("[v]");
    if has_audio {
        cmd.arg("-map").arg("[aout]");
    } else {
        cmd.arg("-map").arg("1:a");
    }
    push_passthrough_streams(&mut cmd, &abs_output);
    push_video_encoder(&mut cmd, enc);
    cmd.arg("-c:a").arg("aac").arg(&abs_output);
    Ok(cmd)
}

/// softsub + dub: video copy, subtitle mux, audio mixed or replaced.
/// Input order: 0=video, 1=srt, 2=dub.wav.
pub fn build_softsub_dub_command(
    ffmpeg: &Path,
    video: &Path,
    srt_path: &Path,
    dub_wav: &Path,
    output: &Path,
    orig_volume: f32,
    has_audio: bool,
) -> anyhow::Result<Command> {
    let codec = new_sub_codec(output)?;

    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(video)
        .arg("-i")
        .arg(srt_path)
        .arg("-i")
        .arg(dub_wav);
    if has_audio {
        cmd.arg("-filter_complex").arg(format!(
            "[0:a]volume={orig_volume}[a0];[2:a]apad[a1];[a0][a1]amix=inputs=2:duration=longest:normalize=0[aout]"
        ));
    }
    // New sub track mapped first so `-c:s:0` targets it; the input's own
    // streams pass through unchanged.
    cmd.arg("-map").arg("1:0").arg("-map").arg("0:v");
    if has_audio {
        cmd.arg("-map").arg("[aout]");
    } else {
        cmd.arg("-map").arg("2:a");
    }
    push_passthrough_streams(&mut cmd, output);
    cmd.arg("-c:v")
        .arg("copy")
        .arg("-c:a")
        .arg("aac")
        .arg("-c:s:0")
        .arg(codec)
        .arg(output);
    Ok(cmd)
}

/// Subtitle codec by output extension: mp4|m4v|mov -> mov_text, mkv -> srt, else Err.
pub fn build_softsub_command(
    ffmpeg: &Path,
    video: &Path,
    srt_path: &Path,
    output: &Path,
) -> anyhow::Result<Command> {
    let codec = new_sub_codec(output)?;

    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(video)
        .arg("-i")
        .arg(srt_path)
        // New sub first (→ s:0); every original stream of input 0 is copied.
        .arg("-map")
        .arg("1:0")
        .arg("-map")
        .arg("0")
        .arg("-map_metadata")
        .arg("0")
        .arg("-map_chapters")
        .arg("0")
        .arg("-c")
        .arg("copy")
        .arg("-c:s")
        .arg(passthrough_sub_codec(output))
        .arg("-c:s:0")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardsub_dub_single_graph() {
        let dir = tempfile::tempdir().unwrap();
        let ass = dir.path().join("subtitle.ass");
        std::fs::write(&ass, "x").unwrap();
        let cmd = build_hardsub_dub_command(
            Path::new("ffmpeg"),
            Path::new("/v.mp4"),
            &ass,
            Path::new("/d.wav"),
            Path::new("/o.mp4"),
            0.15,
            true,
            0,
            EncoderOpts {
                nvenc: false,
                speed: RenderSpeed::Balanced,
                crf: 20,
            },
        )
        .unwrap();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into()).collect();
        let fc = args[args.iter().position(|a| a == "-filter_complex").unwrap() + 1].clone();
        assert!(fc.contains("ass=subtitle.ass"));
        assert!(fc.contains("volume=0.15"));
        assert!(fc.contains("amix=inputs=2"));
        assert!(args.contains(&"-c:a".to_string()) && args.contains(&"aac".to_string()));
    }

    #[test]
    fn softsub_dub_maps_subtitle_and_mixes() {
        let cmd = build_softsub_dub_command(
            Path::new("ffmpeg"),
            Path::new("/v.mp4"),
            Path::new("/s.srt"),
            Path::new("/d.wav"),
            Path::new("/o.mkv"),
            0.2,
            true,
        )
        .unwrap();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into()).collect();
        let cs = args.iter().position(|a| a == "-c:s:0").unwrap();
        assert_eq!(args[cs + 1], "srt"); // .mkv → srt codec for injected track
        let fc = args[args.iter().position(|a| a == "-filter_complex").unwrap() + 1].clone();
        assert!(fc.contains("[2:a]apad"));
    }

    #[test]
    fn no_audio_maps_dub_directly() {
        let cmd = build_softsub_dub_command(
            Path::new("ffmpeg"),
            Path::new("/v.mp4"),
            Path::new("/s.srt"),
            Path::new("/d.wav"),
            Path::new("/o.mp4"),
            0.15,
            false,
        )
        .unwrap();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into()).collect();
        assert!(!args.contains(&"-filter_complex".to_string()));
        assert!(args.contains(&"2:a".to_string()));
    }
}
