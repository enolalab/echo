use async_trait::async_trait;
use auto_subtitle::{
    cli::Cli, pipeline::*, stt::Transcriber, subtitle::model::SubtitleSegment as S,
    translate::Translator,
};
use clap::Parser;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MockStt;
#[async_trait]
impl Transcriber for MockStt {
    async fn transcribe(&self, _: &std::path::Path) -> anyhow::Result<Vec<S>> {
        Ok(vec![
            S::new(1, 0, 1000, "Hello"),
            S::new(2, 1000, 2500, "World"),
        ])
    }
}
struct Prefix;
#[async_trait]
impl Translator for Prefix {
    async fn translate_batch(&self, s: &[S], _: &str, _: &str) -> anyhow::Result<Vec<String>> {
        Ok(s.iter()
            .map(|x| format!("VI:{}", x.original_text))
            .collect())
    }
}
struct Flaky(AtomicUsize);
#[async_trait]
impl Translator for Flaky {
    async fn translate_batch(&self, s: &[S], a: &str, b: &str) -> anyhow::Result<Vec<String>> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            anyhow::bail!("transient")
        }
        Prefix.translate_batch(s, a, b).await
    }
}
struct AlwaysFail;
#[async_trait]
impl Translator for AlwaysFail {
    async fn translate_batch(&self, _: &[S], _: &str, _: &str) -> anyhow::Result<Vec<String>> {
        anyhow::bail!("down")
    }
}

fn many(n: usize) -> Vec<S> {
    (0..n)
        .map(|i| {
            S::new(
                i + 1,
                i as u64 * 1000,
                i as u64 * 1000 + 900,
                format!("l{i}"),
            )
        })
        .collect()
}

#[tokio::test]
async fn translate_all_covers_every_batch() {
    let mut s = many(45); // 3 batches: catches the old global-id bug
    translate_all(&Prefix, &mut s, "en", "vi").await.unwrap();
    assert!(s
        .iter()
        .all(|x| x.translated_text.as_deref() == Some(&format!("VI:{}", x.original_text)[..])));
}

#[tokio::test]
async fn test_translate_all_invokes_progress_callback() {
    let mut s = many(45);
    let mut recorded = Vec::new();
    translate_all_with_progress(
        &Prefix,
        &mut s,
        "en",
        "vi",
        1,
        Some(|done, total| {
            recorded.push((done, total));
        }),
    )
    .await
    .unwrap();

    assert_eq!(recorded, vec![(20, 45), (40, 45), (45, 45)]);
}

#[tokio::test]
async fn translate_all_retries_then_fails_loudly() {
    let mut s = many(3);
    translate_all(&Flaky(AtomicUsize::new(0)), &mut s, "en", "vi")
        .await
        .unwrap();
    let err = translate_all(&AlwaysFail, &mut many(3), "en", "vi")
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("lines 1–3"));
}

/// Mimics a small local model that drops lines when the batch is too big.
struct DropsLinesAbove(usize);
#[async_trait]
impl Translator for DropsLinesAbove {
    async fn translate_batch(&self, s: &[S], a: &str, b: &str) -> anyhow::Result<Vec<String>> {
        if s.len() > self.0 {
            anyhow::bail!("Missing translated line [{}] in translation response", self.0 + 1);
        }
        Prefix.translate_batch(s, a, b).await
    }
}

#[tokio::test]
async fn dropped_lines_split_batch_until_success() {
    let mut s = many(20); // one 20-line batch: 20 > 5 fails, splits to 10/10, still fails, splits to 5
    translate_all(&DropsLinesAbove(5), &mut s, "en", "vi")
        .await
        .unwrap();
    assert!(s
        .iter()
        .all(|x| x.translated_text.as_deref() == Some(&format!("VI:{}", x.original_text)[..])));
}

#[tokio::test]
async fn unsplittable_parse_failure_still_fails_loudly() {
    let err = translate_all(&DropsLinesAbove(0), &mut many(2), "en", "vi")
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("lines 1–2"));
}

#[tokio::test]
async fn subs_only_with_existing_subtitle_never_touches_input() {
    let t = tempfile::tempdir().unwrap();
    let video = t.path().join("clip.mp4");
    std::fs::write(&video, b"not a real video").unwrap();
    let sub = t.path().join("clip.srt");
    let original = "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n";
    std::fs::write(&sub, original).unwrap();
    let cli = Cli::try_parse_from([
        "a",
        "-i",
        video.to_str().unwrap(),
        "--sub",
        sub.to_str().unwrap(),
        "--mode",
        "subs-only",
        "-b",
    ])
    .unwrap();
    let r = run(
        &cli,
        Deps {
            ffmpeg: None,
            transcriber: None,
            translator: Some(&Prefix),
            tts: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read_to_string(&sub).unwrap(), original); // input untouched
    let out_srt = t.path().join("clip_subtitled.vi.srt");
    assert!(r.outputs.contains(&out_srt));
    assert!(std::fs::read_to_string(out_srt)
        .unwrap()
        .contains("Hello\nVI:Hello"));
}

#[test]
fn output_equal_to_input_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    let video = t.path().join("v.mp4");
    std::fs::write(&video, b"x").unwrap();
    let cli = Cli::try_parse_from([
        "a",
        "-i",
        video.to_str().unwrap(),
        "-o",
        video.to_str().unwrap(),
    ])
    .unwrap();
    assert!(resolve_output_paths(&cli).is_err());
}

struct LlmFails(AtomicUsize, auto_subtitle::llm::LlmError, usize);
#[async_trait]
impl Translator for LlmFails {
    async fn translate_batch(&self, s: &[S], a: &str, b: &str) -> anyhow::Result<Vec<String>> {
        let n = self.0.fetch_add(1, Ordering::SeqCst);
        if n < self.2 {
            return Err(anyhow::Error::new(self.1.clone()));
        }
        Prefix.translate_batch(s, a, b).await
    }
}

#[tokio::test]
async fn auth_error_stops_without_retry() {
    let t = LlmFails(AtomicUsize::new(0), auto_subtitle::llm::LlmError::Auth, usize::MAX);
    let err = translate_all(&t, &mut many(3), "en", "vi").await.unwrap_err();
    assert_eq!(t.0.load(Ordering::SeqCst), 1);
    assert!(format!("{err:#}").contains("API key không hợp lệ"));
}

#[tokio::test(start_paused = true)]
async fn rate_limit_backs_off_then_succeeds() {
    let t = LlmFails(AtomicUsize::new(0), auto_subtitle::llm::LlmError::RateLimit, 2);
    let start = tokio::time::Instant::now();
    translate_all(&t, &mut many(3), "en", "vi").await.unwrap();
    assert_eq!(t.0.load(Ordering::SeqCst), 3);
    assert!(start.elapsed() >= std::time::Duration::from_secs(6));
}

/// Real FFmpeg E2E with mock AI. Skips (passes) when ffmpeg is not installed.
#[tokio::test]
async fn hardsub_and_softsub_e2e_with_synthetic_video() {
    let Ok(ffmpeg) = which::which("ffmpeg") else {
        eprintln!("skip: ffmpeg not found");
        return;
    };
    let t = tempfile::tempdir().unwrap();
    let video = t.path().join("syn.mp4");
    let st = std::process::Command::new(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=3:size=320x240:rate=25",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=3",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&video)
        .status()
        .unwrap();
    assert!(st.success());
    for mode in ["hardsub", "softsub"] {
        let out = t.path().join(format!("out_{mode}.mp4"));
        let cli = Cli::try_parse_from([
            "a",
            "-i",
            video.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--mode",
            mode,
        ])
        .unwrap();
        run(
            &cli,
            Deps {
                ffmpeg: Some(ffmpeg.clone()),
                transcriber: Some(&MockStt),
                translator: Some(&Prefix),
                tts: None,
            },
        )
        .await
        .unwrap();
        assert!(
            std::fs::metadata(&out).unwrap().len() > 0,
            "{mode} output missing"
        );
    }
}
