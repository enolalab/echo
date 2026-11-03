use auto_subtitle::media::*;
use std::path::Path;

fn args(c: &std::process::Command) -> Vec<String> {
    c.get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn wav_extract_args() {
    let a = args(&build_extract_wav_command(
        Path::new("ffmpeg"),
        Path::new("in.mp4"),
        Path::new("a.wav"),
    ));
    assert!(a.windows(2).any(|w| w == ["-ar", "16000"]));
    assert!(a.windows(2).any(|w| w == ["-ac", "1"]));
    assert!(a.windows(2).any(|w| w == ["-c:a", "pcm_s16le"]));
}

#[test]
fn mp3_extract_args() {
    let a = args(&build_extract_mp3_command(
        Path::new("ffmpeg"),
        Path::new("in.mp4"),
        Path::new("a.mp3"),
    ));
    assert!(a.windows(2).any(|w| w == ["-ar", "16000"]));
    assert!(a.windows(2).any(|w| w == ["-ac", "1"]));
    assert!(a.windows(2).any(|w| w == ["-c:a", "libmp3lame"]));
    assert!(a.windows(2).any(|w| w == ["-b:a", "32k"]));
}

#[test]
fn hardsub_uses_workdir_and_safe_filter() {
    let tmp = tempfile::tempdir().unwrap();
    let ass = tmp.path().join("subtitle.ass");
    let cmd = build_hardsub_command(
        Path::new("ffmpeg"),
        Path::new("My Clip: 1.mp4"),
        &ass,
        Path::new("out.mp4"),
        0,
        EncoderOpts {
            nvenc: false,
            speed: RenderSpeed::Balanced,
            crf: 20,
            max_bitrate_kbps: None,
        },
    )
    .unwrap();
    assert_eq!(cmd.get_current_dir(), Some(tmp.path()));
    let a = args(&cmd);
    let fc = a[a.iter().position(|x| x == "-filter_complex").unwrap() + 1].clone();
    assert!(fc.contains("ass=subtitle.ass"));
    assert!(a
        .iter()
        .any(|x| x.ends_with("My Clip: 1.mp4") && Path::new(x).is_absolute()));
}

#[test]
fn hardsub_rejects_unsafe_ass_name() {
    assert!(build_hardsub_command(
        Path::new("ffmpeg"),
        Path::new("a.mp4"),
        Path::new("/tmp/a b:c.ass"),
        Path::new("o.mp4"),
        0,
        EncoderOpts {
            nvenc: false,
            speed: RenderSpeed::Balanced,
            crf: 20,
            max_bitrate_kbps: None
        }
    )
    .is_err());
}

#[test]
fn hardsub_pad_extends_frame_then_burns() {
    let tmp = tempfile::tempdir().unwrap();
    let ass = tmp.path().join("subtitle.ass");
    let cmd = build_hardsub_command(
        Path::new("ffmpeg"),
        Path::new("in.mp4"),
        &ass,
        Path::new("out.mp4"),
        240,
        EncoderOpts {
            nvenc: false,
            speed: RenderSpeed::Balanced,
            crf: 20,
            max_bitrate_kbps: None,
        },
    )
    .unwrap();
    let a = args(&cmd);
    let fc = a[a.iter().position(|x| x == "-filter_complex").unwrap() + 1].clone();
    assert_eq!(fc, "[0:v]pad=iw:ih+240:0:0:black,ass=subtitle.ass[v]");
}

#[test]
fn hardsub_pad_odd_height_rounded_down() {
    let tmp = tempfile::tempdir().unwrap();
    let ass = tmp.path().join("subtitle.ass");
    let cmd = build_hardsub_command(
        Path::new("ffmpeg"),
        Path::new("in.mp4"),
        &ass,
        Path::new("out.mp4"),
        201,
        EncoderOpts {
            nvenc: false,
            speed: RenderSpeed::Balanced,
            crf: 20,
            max_bitrate_kbps: None,
        },
    )
    .unwrap();
    let a = args(&cmd);
    let fc = a[a.iter().position(|x| x == "-filter_complex").unwrap() + 1].clone();
    assert!(fc.contains("ih+200"), "odd pad must round to even: {fc}");
}

#[test]
fn parse_video_size_from_stderr() {
    let stderr = "Input #0, mov,mp4, from 'in.mp4':\n  Stream #0:0(und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 1920x1080 [SAR 1:1 DAR 16:9], 24 fps\n  Stream #0:1(und): Audio: aac (LC), 48000 Hz";
    assert_eq!(parse_video_size(stderr), Some((1920, 1080)));
    assert_eq!(parse_video_size("no streams"), None);
}

#[test]
fn parse_video_bitrate_prefers_stream_line() {
    let stderr = "  Duration: 00:06:44.00, bitrate: 690 kb/s\n  Stream #0:0(und): Video: h264 (High), yuv420p, 1920x1080, 554 kb/s, 30 fps\n  Stream #0:1(und): Audio: aac, 44100 Hz, 127 kb/s";
    assert_eq!(parse_video_bitrate_kbps(stderr), Some(554));
    // No per-stream bitrate → 85% of container bitrate.
    let stderr2 =
        "  Duration: 00:01:00.00, bitrate: 1000 kb/s\n  Stream #0:0: Video: vp9, 1920x1080, 30 fps";
    assert_eq!(parse_video_bitrate_kbps(stderr2), Some(850));
    assert_eq!(parse_video_bitrate_kbps("no streams"), None);
}

#[test]
fn hardsub_pushes_maxrate_and_bufsize() {
    let tmp = tempfile::tempdir().unwrap();
    let ass = tmp.path().join("subtitle.ass");
    let cmd = build_hardsub_command(
        Path::new("ffmpeg"),
        Path::new("in.mp4"),
        &ass,
        Path::new("out.mp4"),
        0,
        EncoderOpts {
            nvenc: false,
            speed: RenderSpeed::Balanced,
            crf: 20,
            max_bitrate_kbps: Some(600),
        },
    )
    .unwrap();
    let a = args(&cmd);
    assert!(a.windows(2).any(|w| w == ["-maxrate", "600k"]));
    assert!(a.windows(2).any(|w| w == ["-bufsize", "1200k"]));
}

#[test]
fn softsub_codec_by_container() {
    let mp4 = args(
        &build_softsub_command(
            Path::new("ffmpeg"),
            Path::new("a.mp4"),
            Path::new("s.srt"),
            Path::new("o.mp4"),
        )
        .unwrap(),
    );
    assert!(mp4.windows(2).any(|w| w == ["-c:s:0", "mov_text"]));
    let mkv = args(
        &build_softsub_command(
            Path::new("ffmpeg"),
            Path::new("a.mkv"),
            Path::new("s.srt"),
            Path::new("o.mkv"),
        )
        .unwrap(),
    );
    assert!(mkv.windows(2).any(|w| w == ["-c:s:0", "srt"]));
    assert!(mkv.windows(2).any(|w| w == ["-c:s", "copy"]));
    assert!(build_softsub_command(
        Path::new("ffmpeg"),
        Path::new("a.mp4"),
        Path::new("s.srt"),
        Path::new("o.avi")
    )
    .is_err());
}

#[test]
fn run_ffmpeg_success() {
    let mut cmd = std::process::Command::new("sh");
    cmd.args(["-c", "exit 0"]);
    assert!(run_ffmpeg(cmd, "test").is_ok());
}

#[test]
fn run_ffmpeg_failure_tail_stderr() {
    let mut cmd = std::process::Command::new("sh");
    cmd.args([
        "-c",
        "for i in $(seq 1 30); do echo line $i >&2; done; exit 1",
    ]);
    let err = run_ffmpeg(cmd, "test-op").unwrap_err().to_string();
    assert!(err.contains("ffmpeg failed for test-op"));
    assert!(err.contains("line 30"));
    assert!(err.contains("line 11"));
    assert!(!err.contains("line 10\n"));
}

#[test]
fn cancel_flag_kills_long_running_child() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let mut cmd = std::process::Command::new("sh");
    cmd.args(["-c", "sleep 60"]);
    let flag = Arc::new(AtomicBool::new(false));
    let flag_setter = flag.clone();

    let start = Instant::now();
    let res = std::thread::scope(|s| {
        s.spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            flag_setter.store(true, Ordering::Relaxed);
        });
        run_ffmpeg_cancellable(cmd, "sleep-op", Some(flag), None)
    });

    let err = res.unwrap_err().to_string();
    assert!(err.contains("cancelled"), "unexpected error: {err}");
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "cancel must kill the child promptly, not wait for it"
    );
}

#[test]
fn kill_all_children_reaps_in_flight_child() {
    use std::time::{Duration, Instant};

    let mut cmd = std::process::Command::new("sh");
    cmd.args(["-c", "sleep 60"]);

    let start = Instant::now();
    let (res,) = std::thread::scope(|s| {
        let handle = s.spawn(move || run_ffmpeg(cmd, "sleep-op"));
        std::thread::sleep(Duration::from_millis(200));
        kill_all_children();
        (handle.join().unwrap(),)
    });

    assert!(res.is_err(), "killed child must surface as an error");
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "kill_all_children must unblock run_ffmpeg"
    );
}

#[test]
fn nvenc_failure_retries_on_cpu() {
    use std::cell::Cell;

    let calls = Cell::new(0);
    let enc = EncoderOpts {
        nvenc: true,
        speed: RenderSpeed::Balanced,
        crf: 20,
        max_bitrate_kbps: None,
    };
    let res = run_encode_with_cpu_fallback(
        enc,
        "enc-op",
        |e| {
            calls.set(calls.get() + 1);
            let mut c = std::process::Command::new("sh");
            c.arg("-c").arg(if e.nvenc { "exit 1" } else { "exit 0" });
            Ok(c)
        },
        None,
        None,
    );
    assert!(res.is_ok(), "cpu retry should succeed: {res:?}");
    assert_eq!(calls.get(), 2, "nvenc fail must trigger exactly one retry");
}

#[test]
fn progress_ticks_feed_callback_and_leave_error_tail() {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    let last = Arc::new(AtomicU64::new(0));
    let cb = {
        let last = last.clone();
        Arc::new(move |us: u64| last.store(us, Ordering::Relaxed)) as Arc<dyn Fn(u64) + Send + Sync>
    };
    // ffmpeg `-progress` format: one key=value per line on stderr.
    let mut cmd = std::process::Command::new("sh");
    cmd.args([
        "-c",
        "echo 'out_time_ms=1250000' >&2; echo 'real error line' >&2; echo 'progress=end' >&2; exit 1",
    ]);
    let err = run_ffmpeg_cancellable(cmd, "p", None, Some(cb)).unwrap_err();
    assert_eq!(last.load(Ordering::Relaxed), 1_250_000);
    let msg = format!("{err:#}");
    assert!(msg.contains("real error line"), "missing: {msg}");
    assert!(
        !msg.contains("out_time_ms"),
        "progress leaked into tail: {msg}"
    );
}

#[test]
fn cancelled_encode_does_not_retry() {
    use std::cell::Cell;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    let calls = Cell::new(0);
    let flag = Arc::new(AtomicBool::new(true)); // already cancelled
    let enc = EncoderOpts {
        nvenc: true,
        speed: RenderSpeed::Balanced,
        crf: 20,
        max_bitrate_kbps: None,
    };
    let res = run_encode_with_cpu_fallback(
        enc,
        "enc-op",
        |e| {
            calls.set(calls.get() + 1);
            let mut c = std::process::Command::new("sh");
            c.arg("-c").arg(if e.nvenc { "exit 1" } else { "exit 0" });
            Ok(c)
        },
        Some(flag),
        None,
    );
    assert!(res.is_err());
    assert_eq!(calls.get(), 1, "cancelled encode must not retry on x264");
}
