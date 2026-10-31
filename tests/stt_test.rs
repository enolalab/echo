use auto_subtitle::stt::{
    audio::read_wav_16k_mono,
    whisper_api::{ApiTranscriber, API_MAX_UPLOAD_BYTES},
    whisper_local::segments_from_whisper,
    Transcriber,
};
use serde_json::json;
use wiremock::{matchers::path, Mock, MockServer, ResponseTemplate};

fn write_wav(p: &std::path::Path, rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(p, spec).unwrap();
    for _ in 0..1600 {
        w.write_sample(16384i16).unwrap();
    }
    w.finalize().unwrap();
}

#[test]
fn reads_valid_wav_and_rejects_wrong_rate() {
    let t = tempfile::tempdir().unwrap();
    let ok = t.path().join("ok.wav");
    write_wav(&ok, 16000);
    let s = read_wav_16k_mono(&ok).unwrap();
    assert_eq!(s.len(), 1600);
    assert!((s[0] - 0.5).abs() < 1e-4);
    let bad = t.path().join("bad.wav");
    write_wav(&bad, 44100);
    assert!(read_wav_16k_mono(&bad).is_err());
}

#[test]
fn whisper_centiseconds_to_ms() {
    let segs = segments_from_whisper(vec![
        (0, 150, " Hello ".into()),
        (150, 150, "empty-dur".into()),
        (200, 420, "World".into()),
    ]);
    assert_eq!(segs.len(), 2);
    assert_eq!(
        (
            segs[0].start_ms,
            segs[0].end_ms,
            segs[0].original_text.as_str()
        ),
        (0, 1500, "Hello")
    );
    assert_eq!(
        (segs[1].id, segs[1].start_ms, segs[1].end_ms),
        (2, 2000, 4200)
    );
}

#[tokio::test]
async fn api_transcriber_parses_verbose_json() {
    let s = MockServer::start().await;
    Mock::given(path("/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "segments": [{"start": 0.0, "end": 1.25, "text": " Hi "}, {"start": 1.25, "end": 2.5, "text": "there"}]
        })))
        .mount(&s)
        .await;
    let t = tempfile::tempdir().unwrap();
    let audio = t.path().join("a.mp3");
    std::fs::write(&audio, b"fake").unwrap();
    let tr = ApiTranscriber::new("k".into(), s.uri(), "whisper-1".into(), None);
    let segs = tr.transcribe(&audio).await.unwrap();
    assert_eq!(
        (segs[0].end_ms, segs[0].original_text.as_str()),
        (1250, "Hi")
    );
    assert_eq!(segs.len(), 2);
}

#[tokio::test]
async fn api_transcriber_rejects_file_over_limit() {
    let t = tempfile::tempdir().unwrap();
    let large = t.path().join("large.mp3");
    let f = std::fs::File::create(&large).unwrap();
    f.set_len(API_MAX_UPLOAD_BYTES + 1).unwrap();
    let tr = ApiTranscriber::new(
        "k".into(),
        "http://localhost:1234".into(),
        "whisper-1".into(),
        None,
    );
    let err = tr.transcribe(&large).await.unwrap_err();
    assert!(err.to_string().contains("--stt-engine local"));
}

#[tokio::test]
async fn api_transcriber_surfaces_http_errors() {
    let s = MockServer::start().await;
    Mock::given(path("/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&s)
        .await;
    let t = tempfile::tempdir().unwrap();
    let audio = t.path().join("a.mp3");
    std::fs::write(&audio, b"fake").unwrap();
    let tr = ApiTranscriber::new("k".into(), s.uri(), "whisper-1".into(), None);
    let err = tr.transcribe(&audio).await.unwrap_err();
    assert!(err.to_string().contains("401") || err.to_string().contains("failed"));
}

#[tokio::test]
async fn api_transcriber_handles_empty_speech() {
    let s = MockServer::start().await;
    Mock::given(path("/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "segments": []
        })))
        .mount(&s)
        .await;
    let t = tempfile::tempdir().unwrap();
    let audio = t.path().join("a.mp3");
    std::fs::write(&audio, b"fake").unwrap();
    let tr = ApiTranscriber::new("k".into(), s.uri(), "whisper-1".into(), None);
    let segs = tr.transcribe(&audio).await.unwrap();
    assert!(segs.is_empty());
}

/// Requires a real model: AUTO_SUBTITLE_TEST_MODEL=/path/ggml-tiny.bin cargo test -- --ignored
#[tokio::test]
#[ignore]
async fn local_whisper_runs_on_silence() {
    use auto_subtitle::stt::whisper_local::LocalWhisperTranscriber;
    let model = std::env::var("AUTO_SUBTITLE_TEST_MODEL").expect("set AUTO_SUBTITLE_TEST_MODEL");
    let t = tempfile::tempdir().unwrap();
    let wav = t.path().join("s.wav");
    write_wav(&wav, 16000);
    let tr = LocalWhisperTranscriber::new(model.into(), None, false);
    tr.transcribe(&wav).await.unwrap(); // must not error; may return empty
}
