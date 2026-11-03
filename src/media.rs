use anyhow::Context;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

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
    /// VBV cap in kb/s (usually the input's video bitrate) so the re-encode
    /// can't balloon past the source's data rate; None = uncapped CRF.
    pub max_bitrate_kbps: Option<u32>,
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
    if let Some(kbps) = enc.max_bitrate_kbps {
        cmd.arg("-maxrate")
            .arg(format!("{kbps}k"))
            .arg("-bufsize")
            .arg(format!("{}k", kbps.saturating_mul(2)));
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

/// Parse the video stream bitrate (kb/s) from `ffmpeg -i` stderr. Falls back
/// to 85% of the container's overall bitrate when the stream line has none.
pub fn parse_video_bitrate_kbps(stderr: &str) -> Option<u32> {
    static RE_STREAM: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static RE_TOTAL: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re_stream = RE_STREAM
        .get_or_init(|| regex::Regex::new(r"Stream[^\n]*Video:[^\n]*?(\d+)\s*kb/s").unwrap());
    if let Some(c) = re_stream.captures(stderr) {
        return c[1].parse().ok();
    }
    let re_total = RE_TOTAL.get_or_init(|| regex::Regex::new(r"bitrate:\s*(\d+)\s*kb/s").unwrap());
    re_total
        .captures(stderr)
        .and_then(|c| c[1].parse::<u32>().ok())
        .map(|kbps| kbps * 85 / 100)
}

/// `ffmpeg -i <video>` probe → video bitrate in kb/s (None if unknown).
pub fn probe_video_bitrate_kbps(ffmpeg: &Path, video: &Path) -> Option<u32> {
    let out = std::process::Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(video)
        .output()
        .ok()?;
    parse_video_bitrate_kbps(&String::from_utf8_lossy(&out.stderr))
}

/// Parse `Duration: HH:MM:SS.cc` from `ffmpeg -i` stderr → milliseconds.
pub fn parse_duration_ms(stderr: &str) -> Option<u64> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re =
        RE.get_or_init(|| regex::Regex::new(r"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)").unwrap());
    let c = re.captures(stderr)?;
    let h: u64 = c[1].parse().ok()?;
    let m: u64 = c[2].parse().ok()?;
    let s: f64 = c[3].parse().ok()?;
    Some((h * 3600 + m * 60) as u64 * 1000 + (s * 1000.0) as u64)
}

/// `ffmpeg -i <video>` probe → container duration in ms (None if unknown).
pub fn probe_duration_ms(ffmpeg: &Path, video: &Path) -> Option<u64> {
    let out = std::process::Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(video)
        .output()
        .ok()?;
    parse_duration_ms(&String::from_utf8_lossy(&out.stderr))
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
        .arg("-y")
        .arg("-progress")
        .arg("pipe:2");
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
        // duration=first bounds the mix to the original audio (≈ video length).
        // An `apad`-style infinite input with duration=longest would make the
        // encode run forever, growing the file with silence past the end.
        format!(
            "{vgraph};[0:a]volume={orig_volume}[a0];[a0][1:a]amix=inputs=2:duration=first:normalize=0[aout]"
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
        .arg("-y")
        .arg("-progress")
        .arg("pipe:2");
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
    cmd.arg("-c:a").arg("aac").arg("-shortest").arg(&abs_output);
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
        .arg("-progress")
        .arg("pipe:2")
        .arg("-i")
        .arg(video)
        .arg("-i")
        .arg(srt_path)
        .arg("-i")
        .arg(dub_wav);
    if has_audio {
        cmd.arg("-filter_complex").arg(format!(
            "[0:a]volume={orig_volume}[a0];[a0][2:a]amix=inputs=2:duration=first:normalize=0[aout]"
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
        .arg("-shortest")
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
        .arg("-progress")
        .arg("pipe:2")
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

/// ffmpeg children currently in flight, keyed by pid. Lets the host app reap
/// them all on exit — a spawned ffmpeg would otherwise outlive the process as
/// an orphan (observed: an encode burned ~1.5h after the app had closed).
fn running_children() -> &'static Mutex<HashMap<u32, Child>> {
    static M: OnceLock<Mutex<HashMap<u32, Child>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Kill every in-flight ffmpeg child. Idempotent; call when the host process
/// exits (or as a last-resort stop for all jobs).
pub fn kill_all_children() {
    if let Ok(mut map) = running_children().lock() {
        for (_, mut c) in map.drain() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Run a re-encode command built by `rebuild(enc)`. When the NVENC path fails
/// at runtime (encoder listed but driver/API/context unusable), retry once on
/// libx264 instead of aborting the export — except on cancel, which stops.
pub fn run_encode_with_cpu_fallback<F>(
    enc: EncoderOpts,
    what: &str,
    rebuild: F,
    cancel: Option<Arc<AtomicBool>>,
    on_progress: Option<Arc<dyn Fn(u64) + Send + Sync>>,
) -> anyhow::Result<()>
where
    F: Fn(EncoderOpts) -> anyhow::Result<Command>,
{
    let cmd = rebuild(enc)?;
    match run_ffmpeg_cancellable(cmd, what, cancel.clone(), on_progress.clone()) {
        Err(e) if enc.nvenc => {
            // A cancelled job must not be retried — killing nvenc would just
            // restart the same encode on x264 for minutes.
            if cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed)) {
                return Err(e);
            }
            let cpu = EncoderOpts {
                nvenc: false,
                ..enc
            };
            run_ffmpeg_cancellable(rebuild(cpu)?, what, cancel, on_progress).map_err(|cpu_e| {
                anyhow::anyhow!("nvenc failed: {e:#}; libx264 retry also failed: {cpu_e:#}")
            })
        }
        r => r,
    }
}

/// Runs the command; on non-zero exit returns Err containing the last 20 stderr lines.
pub fn run_ffmpeg(cmd: Command, what: &str) -> anyhow::Result<()> {
    run_ffmpeg_cancellable(cmd, what, None, None)
}

/// `-progress` lines ffmpeg writes — parsed for the callback, kept out of the
/// error tail so real diagnostics aren't pushed out by progress spam.
fn is_progress_line(l: &str) -> bool {
    const KEYS: [&str; 11] = [
        "frame=",
        "fps=",
        "stream_",
        "bitrate=",
        "total_size=",
        "out_time_us=",
        "out_time_ms=",
        "out_time=",
        "dup_frames=",
        "drop_frames=",
        "speed=",
    ];
    l.starts_with("progress=") || KEYS.iter().any(|k| l.starts_with(k))
}

/// Runs the command, killing the child promptly when `cancel` flips true.
/// `on_progress` receives `out_time_us` ticks when the command was built with
/// `-progress pipe:2`. On non-zero exit returns Err with the stderr tail.
pub fn run_ffmpeg_cancellable(
    mut cmd: Command,
    what: &str,
    cancel: Option<Arc<AtomicBool>>,
    on_progress: Option<Arc<dyn Fn(u64) + Send + Sync>>,
) -> anyhow::Result<()> {
    if cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed)) {
        anyhow::bail!("{what} cancelled");
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to execute ffmpeg for {what}"))?;
    let pid = child.id();
    // Drain stderr on a side thread: while we poll try_wait, a filled pipe
    // buffer would otherwise block the child. Delivered over a channel —
    // recv_timeout'd below, because a grandchild inheriting the pipe would
    // keep read_to_end (and thus a plain join) blocked past the kill.
    let stderr_pipe = child.stderr.take().expect("stderr is piped");
    let (stderr_tx, stderr_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut rd = std::io::BufReader::new(stderr_pipe);
        let mut buf = Vec::new();
        let mut line = String::new();
        loop {
            line.clear();
            match rd.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let l = line.trim_end();
                    if let Some(us) = l
                        .strip_prefix("out_time_ms=")
                        .and_then(|v| v.parse::<u64>().ok())
                    {
                        if let Some(cb) = &on_progress {
                            cb(us);
                        }
                    }
                    if !is_progress_line(l) {
                        buf.extend_from_slice(l.as_bytes());
                        buf.push(b'\n');
                    }
                }
            }
        }
        let _ = stderr_tx.send(buf);
    });
    if let Ok(mut m) = running_children().lock() {
        m.insert(pid, child);
    }

    let mut cancelled = false;
    let status = loop {
        let done = {
            let mut map = running_children().lock().unwrap();
            match map.get_mut(&pid) {
                // Entry reaped by kill_all_children → the child is gone.
                None => break None,
                Some(c) => c.try_wait(),
            }
        };
        match done {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {}
            Err(e) => {
                running_children().lock().unwrap().remove(&pid);
                return Err(e).context(format!("failed to wait on ffmpeg for {what}"));
            }
        }
        if cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed)) {
            if let Ok(mut map) = running_children().lock() {
                if let Some(mut c) = map.remove(&pid) {
                    let _ = c.kill();
                    let _ = c.wait();
                }
            }
            cancelled = true;
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    running_children().lock().unwrap().remove(&pid);
    let buf = stderr_rx
        .recv_timeout(std::time::Duration::from_millis(500))
        .unwrap_or_default();

    if cancelled {
        anyhow::bail!("{what} cancelled");
    }
    let status = status.with_context(|| format!("ffmpeg for {what} was killed"))?;
    if !status.success() {
        let stderr = String::from_utf8_lossy(&buf);
        let lines: Vec<&str> = stderr.lines().collect();
        let tail_lines = if lines.len() > 20 {
            &lines[lines.len() - 20..]
        } else {
            &lines[..]
        };
        let tail = tail_lines.join("\n");
        anyhow::bail!(
            "ffmpeg failed for {what} (exit code: {}):\n{tail}",
            status
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
                max_bitrate_kbps: Some(600),
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
        assert!(fc.contains("[a0][2:a]amix"));
        assert!(fc.contains("duration=first"), "amix must not run forever");
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
