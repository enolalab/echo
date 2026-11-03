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
            crf: 20
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
