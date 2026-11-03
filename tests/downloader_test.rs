use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use auto_subtitle::{config::AppConfig, downloader::*};
use wiremock::{matchers::path, Mock, MockServer, ResponseTemplate};

fn cfg() -> (tempfile::TempDir, AppConfig) {
    let t = tempfile::tempdir().unwrap();
    let c = AppConfig::with_base(t.path().to_path_buf()).unwrap();
    (t, c)
}

#[test]
fn rejects_unknown_model_names() {
    let (_t, c) = cfg();
    let m = DependencyManager::new(c);
    assert!(m.model_path("small").unwrap().ends_with("ggml-small.bin"));
    assert!(m.model_path("../../etc/passwd").is_err());
}

#[tokio::test]
async fn downloads_model_atomically() {
    let server = MockServer::start().await;
    Mock::given(path("/ggml-tiny.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![7u8; 1024]))
        .mount(&server)
        .await;
    let (_t, c) = cfg();
    let m = DependencyManager::with_urls(c.clone(), server.uri(), "unused".into());
    let p = m.ensure_whisper_model("tiny").await.unwrap();
    assert_eq!(std::fs::read(&p).unwrap().len(), 1024);
    assert!(!p.with_extension("bin.part").exists());
}

#[tokio::test]
async fn http_error_leaves_no_file() {
    let server = MockServer::start().await;
    Mock::given(path("/ggml-base.bin"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let (_t, c) = cfg();
    let m = DependencyManager::with_urls(c.clone(), server.uri(), "unused".into());
    assert!(m.ensure_whisper_model("base").await.is_err());
    assert!(!c.models_dir.join("ggml-base.bin").exists());
    assert!(!c.models_dir.join("ggml-base.bin.part").exists());
}

#[tokio::test]
async fn existing_non_empty_model_skips_download() {
    let server = MockServer::start().await;
    // Server returns 500 if hit
    Mock::given(path("/ggml-tiny.bin"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let (_t, c) = cfg();
    let existing_path = c.models_dir.join("ggml-tiny.bin");
    fs::write(&existing_path, b"already downloaded").unwrap();

    let m = DependencyManager::with_urls(c.clone(), server.uri(), "unused".into());
    let p = m.ensure_whisper_model("tiny").await.unwrap();
    assert_eq!(p, existing_path);
    assert_eq!(fs::read(&p).unwrap(), b"already downloaded");
}

#[tokio::test]
async fn existing_empty_model_triggers_re_download() {
    let server = MockServer::start().await;
    Mock::given(path("/ggml-tiny.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![42u8; 512]))
        .mount(&server)
        .await;
    let (_t, c) = cfg();
    let existing_path = c.models_dir.join("ggml-tiny.bin");
    fs::write(&existing_path, b"").unwrap();

    let m = DependencyManager::with_urls(c.clone(), server.uri(), "unused".into());
    let p = m.ensure_whisper_model("tiny").await.unwrap();
    assert_eq!(p, existing_path);
    assert_eq!(fs::read(&p).unwrap().len(), 512);
}

#[tokio::test]
async fn finds_ffmpeg_in_bin_dir() {
    let (_t, c) = cfg();
    let bin_ffmpeg = c.bin_dir.join("ffmpeg");
    fs::write(&bin_ffmpeg, b"#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    fs::set_permissions(&bin_ffmpeg, fs::Permissions::from_mode(0o755)).unwrap();

    let m = DependencyManager::with_urls(c.clone(), "unused".into(), "unused".into());
    // Since which("ffmpeg") is not on PATH (or even if it is, if bin_ffmpeg exists),
    // let's test lookup logic
    if which::which("ffmpeg").is_err() {
        let path = m.find_or_download_ffmpeg().await.unwrap();
        assert_eq!(path, bin_ffmpeg);
    }
}

#[tokio::test]
async fn downloads_and_extracts_ffmpeg_tar_xz() {
    if which::which("ffmpeg").is_ok() {
        // System ffmpeg is installed, which("ffmpeg") will return first
        return;
    }

    // Build a dummy tar.xz containing ffmpeg-release/ffmpeg
    let staging = tempfile::tempdir().unwrap();
    let sub = staging.path().join("ffmpeg-release-amd64-static");
    fs::create_dir_all(&sub).unwrap();
    let script_path = sub.join("ffmpeg");
    fs::write(
        &script_path,
        b"#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then echo \"ffmpeg version 7.0\"; exit 0; fi\nexit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();

    let tar_path = staging.path().join("archive.tar.xz");
    let status = Command::new("tar")
        .args([
            "-cJf",
            tar_path.to_str().unwrap(),
            "-C",
            staging.path().to_str().unwrap(),
            "ffmpeg-release-amd64-static",
        ])
        .status()
        .expect("tar command must succeed");
    assert!(status.success());

    let archive_bytes = fs::read(&tar_path).unwrap();

    let server = MockServer::start().await;
    Mock::given(path("/ffmpeg.tar.xz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive_bytes))
        .mount(&server)
        .await;

    let (_t, c) = cfg();
    let ffmpeg_url = format!("{}/ffmpeg.tar.xz", server.uri());
    let m = DependencyManager::with_urls(c.clone(), "unused".into(), ffmpeg_url);

    let ffmpeg = m.find_or_download_ffmpeg().await.unwrap();
    assert_eq!(ffmpeg, c.bin_dir.join("ffmpeg"));
    assert!(ffmpeg.is_file());
}

#[tokio::test]
async fn extracts_ffmpeg_from_nested_bin_layout() {
    // BtbN archives ship the binary at <dir>/bin/ffmpeg, not <dir>/ffmpeg.
    let staging = tempfile::tempdir().unwrap();
    let sub = staging.path().join("ffmpeg-n8.1-linux64-gpl").join("bin");
    fs::create_dir_all(&sub).unwrap();
    let script_path = sub.join("ffmpeg");
    fs::write(
        &script_path,
        b"#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then echo \"ffmpeg version 8.1\"; exit 0; fi\nexit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();

    let tar_path = staging.path().join("archive.tar.xz");
    let status = Command::new("tar")
        .args([
            "-cJf",
            tar_path.to_str().unwrap(),
            "-C",
            staging.path().to_str().unwrap(),
            "ffmpeg-n8.1-linux64-gpl",
        ])
        .status()
        .expect("tar command must succeed");
    assert!(status.success());

    let archive_bytes = fs::read(&tar_path).unwrap();

    let server = MockServer::start().await;
    Mock::given(path("/ffmpeg.tar.xz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(archive_bytes))
        .mount(&server)
        .await;

    let (_t, c) = cfg();
    let ffmpeg_url = format!("{}/ffmpeg.tar.xz", server.uri());
    let m = DependencyManager::with_urls(c.clone(), "unused".into(), ffmpeg_url);

    let ffmpeg = m.find_or_download_ffmpeg().await.unwrap();
    assert_eq!(ffmpeg, c.bin_dir.join("ffmpeg"));
    assert!(ffmpeg.is_file());
}
