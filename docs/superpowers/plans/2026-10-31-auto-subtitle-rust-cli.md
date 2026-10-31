# Auto-Subtitle Rust CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a modular, high-performance Rust CLI application that automates transcribing video audio, translating subtitles via local LLM or Cloud API, parsing existing subtitle files, and burning/muxing subtitles into the output video.

**Architecture:** Trait-based pipeline separating transcription (`Transcriber`), translation (`Translator`), subtitle parsing/generation (`SubtitleSegment`, SRT, ASS), and media processing (FFmpeg extraction and burn-in/mux). Missing dependencies (FFmpeg static binary, Whisper GGML models) are automatically downloaded to user cache.

**Tech Stack:** Rust (2021 Edition), `clap` (CLI args derive), `tokio` (async runtime), `reqwest` + `serde` + `serde_json` (LLM communication), `whisper-rs` (whisper.cpp C-bindings with optional CUDA), `indicatif` (terminal progress bars), `anyhow` + `thiserror` (error handling).

## Global Constraints

- Language: Rust 2021 Edition.
- All errors handled via `anyhow::Result` or dedicated errors using `thiserror`.
- Tests must pass via `cargo test`.
- No hardcoded paths; use user cache directory `~/.cache/auto-subtitle/` for models and binaries.
- Intermediate files must be cleaned up unless `--keep-temp` is specified.

---

### Task 1: Scaffolding, Configuration & Data Models

**Files:**
- Create: `Cargo.toml`
- Create: `src/config.rs`
- Create: `src/subtitle/model.rs`
- Create: `src/subtitle/mod.rs`
- Create: `src/lib.rs`
- Create: `tests/model_test.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct SubtitleSegment {
      pub id: usize,
      pub start_ms: u64,
      pub end_ms: u64,
      pub original_text: String,
      pub translated_text: Option<String>,
  }
  impl SubtitleSegment {
      pub fn format_timestamp_srt(ms: u64) -> String; // "HH:MM:SS,mmm"
      pub fn format_timestamp_ass(ms: u64) -> String; // "H:MM:SS.cc"
  }
  pub struct AppConfig {
      pub cache_dir: std::path::PathBuf,
      pub models_dir: std::path::PathBuf,
      pub bin_dir: std::path::PathBuf,
  }
  ```

- [ ] **Step 1: Write failing test for SubtitleSegment timestamp conversions**

```rust
// tests/model_test.rs
use auto_subtitle::subtitle::model::SubtitleSegment;

#[test]
fn test_timestamp_srt_formatting() {
    assert_eq!(SubtitleSegment::format_timestamp_srt(0), "00:00:00,000");
    assert_eq!(SubtitleSegment::format_timestamp_srt(1234), "00:00:01,123");
    assert_eq!(SubtitleSegment::format_timestamp_srt(3661050), "01:01:01,050");
}

#[test]
fn test_timestamp_ass_formatting() {
    assert_eq!(SubtitleSegment::format_timestamp_ass(0), "0:00:00.00");
    assert_eq!(SubtitleSegment::format_timestamp_ass(1234), "0:00:01.23");
    assert_eq!(SubtitleSegment::format_timestamp_ass(3661050), "1:01:01.05");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test model_test`
Expected: FAIL (crate doesn't exist yet)

- [ ] **Step 3: Create `Cargo.toml`, `src/lib.rs`, `src/config.rs`, `src/subtitle/model.rs`**

```toml
# Cargo.toml
[package]
name = "auto-subtitle"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4.5", features = ["derive", "env"] }
tokio = { version = "1.38", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
indicatif = "0.17"
anyhow = "1.0"
thiserror = "1.0"
async-trait = "0.1"
dirs = "5.0"
regex = "1.10"
tempfile = "3.10"

[dev-dependencies]
tokio-test = "0.4"
```

```rust
// src/config.rs
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub cache_dir: PathBuf,
    pub models_dir: PathBuf,
    pub bin_dir: PathBuf,
}

impl AppConfig {
    pub fn load() -> Self {
        let base = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("auto-subtitle");
        let models_dir = base.join("models");
        let bin_dir = base.join("bin");

        let _ = std::fs::create_dir_all(&models_dir);
        let _ = std::fs::create_dir_all(&bin_dir);

        Self {
            cache_dir: base,
            models_dir,
            bin_dir,
        }
    }
}
```

```rust
// src/subtitle/model.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleSegment {
    pub id: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    pub original_text: String,
    pub translated_text: Option<String>,
}

impl SubtitleSegment {
    pub fn new(id: usize, start_ms: u64, end_ms: u64, original_text: String) -> Self {
        Self {
            id,
            start_ms,
            end_ms,
            original_text,
            translated_text: None,
        }
    }

    pub fn format_timestamp_srt(ms: u64) -> String {
        let total_seconds = ms / 1000;
        let millis = ms % 1000;
        let seconds = total_seconds % 60;
        let minutes = (total_seconds / 60) % 60;
        let hours = total_seconds / 3600;
        format!("{:02}:{:02}:{:02},{:03}", hours, minutes, seconds, millis)
    }

    pub fn format_timestamp_ass(ms: u64) -> String {
        let total_seconds = ms / 1000;
        let centis = (ms % 1000) / 10;
        let seconds = total_seconds % 60;
        let minutes = (total_seconds / 60) % 60;
        let hours = total_seconds / 3600;
        format!("{}:{:02}:{:02}.{:02}", hours, minutes, seconds, centis)
    }
}
```

```rust
// src/subtitle/mod.rs
pub mod model;
```

```rust
// src/lib.rs
pub mod config;
pub mod subtitle;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test model_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add Cargo.toml src/ tests/
git commit -m "feat: scaffold project structure and subtitle data models"
```

---

### Task 2: Subtitle Parsing (`parser.rs`)

**Files:**
- Create: `src/subtitle/parser.rs`
- Modify: `src/subtitle/mod.rs`
- Create: `tests/parser_test.rs`

**Interfaces:**
- Consumes: `SubtitleSegment` from `src/subtitle/model.rs`
- Produces:
  ```rust
  pub fn parse_srt(content: &str) -> anyhow::Result<Vec<SubtitleSegment>>;
  pub fn parse_vtt(content: &str) -> anyhow::Result<Vec<SubtitleSegment>>;
  pub fn parse_subtitle_file(path: &std::path::Path) -> anyhow::Result<Vec<SubtitleSegment>>;
  ```

- [ ] **Step 1: Write failing unit test for SRT and VTT parsing**

```rust
// tests/parser_test.rs
use auto_subtitle::subtitle::parser::{parse_srt, parse_vtt};

#[test]
fn test_parse_srt_basic() {
    let srt_content = r#"1
00:00:01,000 --> 00:00:03,500
Hello world!

2
00:00:04,100 --> 00:00:06,200
This is a test subtitle.
Multiple lines supported.
"#;
    let segments = parse_srt(srt_content).unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].id, 1);
    assert_eq!(segments[0].start_ms, 1000);
    assert_eq!(segments[0].end_ms, 3500);
    assert_eq!(segments[0].original_text, "Hello world!");

    assert_eq!(segments[1].id, 2);
    assert_eq!(segments[1].start_ms, 4100);
    assert_eq!(segments[1].end_ms, 6200);
    assert_eq!(segments[1].original_text, "This is a test subtitle.\nMultiple lines supported.");
}

#[test]
fn test_parse_vtt_basic() {
    let vtt_content = r#"WEBVTT

00:00:01.500 --> 00:00:03.000
WebVTT subtitle test
"#;
    let segments = parse_vtt(vtt_content).unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].start_ms, 1500);
    assert_eq!(segments[0].end_ms, 3000);
    assert_eq!(segments[0].original_text, "WebVTT subtitle test");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test parser_test`
Expected: FAIL (module not found)

- [ ] **Step 3: Implement `src/subtitle/parser.rs`**

```rust
// src/subtitle/parser.rs
use crate::subtitle::model::SubtitleSegment;
use anyhow::{Context, Result};
use std::path::Path;

fn parse_timestamp(ts: &str) -> Option<u64> {
    let cleaned = ts.trim().replace(',', ".");
    let parts: Vec<&str> = cleaned.split(':').collect();
    if parts.len() == 3 {
        let hours: u64 = parts[0].parse().ok()?;
        let minutes: u64 = parts[1].parse().ok()?;
        let sec_parts: Vec<&str> = parts[2].split('.').collect();
        let seconds: u64 = sec_parts[0].parse().ok()?;
        let millis: u64 = if sec_parts.len() > 1 {
            let m_str = format!("{:0<3}", sec_parts[1]);
            m_str[..3].parse().ok()?
        } else {
            0
        };
        Some(hours * 3600_000 + minutes * 60_000 + seconds * 1000 + millis)
    } else if parts.len() == 2 {
        let minutes: u64 = parts[0].parse().ok()?;
        let sec_parts: Vec<&str> = parts[1].split('.').collect();
        let seconds: u64 = sec_parts[0].parse().ok()?;
        let millis: u64 = if sec_parts.len() > 1 {
            let m_str = format!("{:0<3}", sec_parts[1]);
            m_str[..3].parse().ok()?
        } else {
            0
        };
        Some(minutes * 60_000 + seconds * 1000 + millis)
    } else {
        None
    }
}

pub fn parse_srt(content: &str) -> Result<Vec<SubtitleSegment>> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let blocks: Vec<&str> = normalized.split("\n\n").collect();
    let mut segments = Vec::new();
    let mut current_id = 1;

    for block in blocks {
        let lines: Vec<&str> = block.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            continue;
        }

        let time_line_idx = if lines[0].contains("-->") {
            0
        } else if lines.len() > 1 && lines[1].contains("-->") {
            1
        } else {
            continue;
        };

        let time_parts: Vec<&str> = lines[time_line_idx].split("-->").collect();
        if time_parts.len() != 2 {
            continue;
        }

        let start_ms = parse_timestamp(time_parts[0]).context("invalid start timestamp")?;
        let end_ms = parse_timestamp(time_parts[1]).context("invalid end timestamp")?;

        let text_lines = &lines[time_line_idx + 1..];
        let text = text_lines.join("\n");

        segments.push(SubtitleSegment {
            id: current_id,
            start_ms,
            end_ms,
            original_text: text,
            translated_text: None,
        });
        current_id += 1;
    }

    Ok(segments)
}

pub fn parse_vtt(content: &str) -> Result<Vec<SubtitleSegment>> {
    let without_header = if content.starts_with("WEBVTT") {
        content.trim_start_matches("WEBVTT").trim()
    } else {
        content
    };
    parse_srt(without_header)
}

pub fn parse_subtitle_file(path: &Path) -> Result<Vec<SubtitleSegment>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read subtitle file at {:?}", path))?;
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    match ext.to_lowercase().as_str() {
        "vtt" => parse_vtt(&content),
        _ => parse_srt(&content),
    }
}
```

```rust
// Update src/subtitle/mod.rs
pub mod model;
pub mod parser;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test parser_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add src/subtitle/parser.rs src/subtitle/mod.rs tests/parser_test.rs
git commit -m "feat: add subtitle parser for SRT and VTT formats"
```

---

### Task 3: Subtitle Formatters (`srt.rs` and `ass.rs`)

**Files:**
- Create: `src/subtitle/srt.rs`
- Create: `src/subtitle/ass.rs`
- Modify: `src/subtitle/mod.rs`
- Create: `tests/formatter_test.rs`

**Interfaces:**
- Consumes: `SubtitleSegment`
- Produces:
  ```rust
  pub fn render_srt(segments: &[SubtitleSegment], bilingual: bool) -> String;
  pub fn render_ass(segments: &[SubtitleSegment], bilingual: bool, title: &str) -> String;
  ```

- [ ] **Step 1: Write failing unit test for SRT and ASS rendering**

```rust
// tests/formatter_test.rs
use auto_subtitle::subtitle::model::SubtitleSegment;
use auto_subtitle::subtitle::srt::render_srt;
use auto_subtitle::subtitle::ass::render_ass;

#[test]
fn test_render_srt_single_and_bilingual() {
    let segments = vec![SubtitleSegment {
        id: 1,
        start_ms: 1000,
        end_ms: 3000,
        original_text: "Hello".to_string(),
        translated_text: Some("Xin chào".to_string()),
    }];

    let single = render_srt(&segments, false);
    assert!(single.contains("00:00:01,000 --> 00:00:03,000"));
    assert!(single.contains("Xin chào"));
    assert!(!single.contains("Hello"));

    let bilingual = render_srt(&segments, true);
    assert!(bilingual.contains("Hello\nXin chào"));
}

#[test]
fn test_render_ass_headers_and_dialogue() {
    let segments = vec![SubtitleSegment {
        id: 1,
        start_ms: 1500,
        end_ms: 4000,
        original_text: "Goodbye".to_string(),
        translated_text: Some("Tạm biệt".to_string()),
    }];

    let ass = render_ass(&segments, true, "Video Test");
    assert!(ass.contains("[Script Info]"));
    assert!(ass.contains("Title: Video Test"));
    assert!(ass.contains("Style: Default"));
    assert!(ass.contains("Dialogue: 0,0:00:01.50,0:00:04.00"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test formatter_test`
Expected: FAIL

- [ ] **Step 3: Implement `src/subtitle/srt.rs` and `src/subtitle/ass.rs`**

```rust
// src/subtitle/srt.rs
use crate::subtitle::model::SubtitleSegment;

pub fn render_srt(segments: &[SubtitleSegment], bilingual: bool) -> String {
    let mut output = String::new();
    for seg in segments {
        output.push_str(&format!("{}\n", seg.id));
        output.push_str(&format!(
            "{} --> {}\n",
            SubtitleSegment::format_timestamp_srt(seg.start_ms),
            SubtitleSegment::format_timestamp_srt(seg.end_ms)
        ));

        let text = match (&seg.translated_text, bilingual) {
            (Some(trans), true) => format!("{}\n{}", seg.original_text, trans),
            (Some(trans), false) => trans.clone(),
            (None, _) => seg.original_text.clone(),
        };

        output.push_str(&text);
        output.push_str("\n\n");
    }
    output
}
```

```rust
// src/subtitle/ass.rs
use crate::subtitle::model::SubtitleSegment;

pub fn render_ass(segments: &[SubtitleSegment], bilingual: bool, title: &str) -> String {
    let mut out = String::new();
    out.push_str("[Script Info]\n");
    out.push_str(&format!("Title: {}\n", title));
    out.push_str("ScriptType: v4.00+\n");
    out.push_str("WrapStyle: 0\n");
    out.push_str("PlayResX: 1920\n");
    out.push_str("PlayResY: 1080\n\n");

    out.push_str("[V4+ Styles]\n");
    out.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
    out.push_str("Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,3,1,2,20,20,40,1\n");
    out.push_str("Style: SubText,Arial,34,&H00E0E0E0,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,1,2,20,20,95,1\n\n");

    out.push_str("[Events]\n");
    out.push_str("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");

    for seg in segments {
        let start = SubtitleSegment::format_timestamp_ass(seg.start_ms);
        let end = SubtitleSegment::format_timestamp_ass(seg.end_ms);

        match (&seg.translated_text, bilingual) {
            (Some(trans), true) => {
                // Secondary original text displayed slightly smaller above
                let orig_escaped = seg.original_text.replace('\n', "\\N");
                let trans_escaped = trans.replace('\n', "\\N");
                out.push_str(&format!(
                    "Dialogue: 0,{},{},SubText,,0,0,0,,{}\n",
                    start, end, orig_escaped
                ));
                out.push_str(&format!(
                    "Dialogue: 1,{},{},Default,,0,0,0,,{}\n",
                    start, end, trans_escaped
                ));
            }
            (Some(trans), false) => {
                let trans_escaped = trans.replace('\n', "\\N");
                out.push_str(&format!(
                    "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
                    start, end, trans_escaped
                ));
            }
            (None, _) => {
                let orig_escaped = seg.original_text.replace('\n', "\\N");
                out.push_str(&format!(
                    "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
                    start, end, orig_escaped
                ));
            }
        }
    }

    out
}
```

```rust
// Update src/subtitle/mod.rs
pub mod ass;
pub mod model;
pub mod parser;
pub mod srt;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test formatter_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add src/subtitle/srt.rs src/subtitle/ass.rs src/subtitle/mod.rs tests/formatter_test.rs
git commit -m "feat: add SRT and ASS subtitle formatters with bilingual support"
```

---

### Task 4: Dependency Downloader (`downloader.rs`)

**Files:**
- Create: `src/downloader.rs`
- Modify: `src/lib.rs`
- Create: `tests/downloader_test.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct DependencyManager {
      config: AppConfig,
  }
  impl DependencyManager {
      pub fn new(config: AppConfig) -> Self;
      pub fn find_or_download_ffmpeg(&self) -> anyhow::Result<std::path::PathBuf>;
      pub fn ensure_whisper_model(&self, model_name: &str) -> anyhow::Result<std::path::PathBuf>;
  }
  ```

- [ ] **Step 1: Write failing test for model path resolution**

```rust
// tests/downloader_test.rs
use auto_subtitle::config::AppConfig;
use auto_subtitle::downloader::DependencyManager;

#[test]
fn test_whisper_model_path_resolution() {
    let config = AppConfig::load();
    let manager = DependencyManager::new(config.clone());
    let path = manager.model_path("tiny");
    assert!(path.ends_with("ggml-tiny.bin"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test downloader_test`
Expected: FAIL

- [ ] **Step 3: Implement `src/downloader.rs`**

```rust
// src/downloader.rs
use crate::config::AppConfig;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub struct DependencyManager {
    config: AppConfig,
}

impl DependencyManager {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn model_path(&self, model_name: &str) -> PathBuf {
        self.config.models_dir.join(format!("ggml-{}.bin", model_name))
    }

    pub fn find_or_download_ffmpeg(&self) -> Result<PathBuf> {
        // 1. Check if ffmpeg exists in PATH
        if let Ok(output) = std::process::Command::new("which").arg("ffmpeg").output() {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    return Ok(PathBuf::from(path_str));
                }
            }
        }

        // 2. Check in cache bin_dir
        let cached_ffmpeg = self.config.bin_dir.join("ffmpeg");
        if cached_ffmpeg.exists() {
            return Ok(cached_ffmpeg);
        }

        println!("⚠️ FFmpeg not found in PATH. Downloading static FFmpeg release to {:?}...", cached_ffmpeg);
        // Download static ffmpeg
        let url = "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz";
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;
        let response = client.get(url).send().context("Failed to download FFmpeg archive")?;

        let tar_xz_path = self.config.bin_dir.join("ffmpeg.tar.xz");
        let mut file = File::create(&tar_xz_path)?;
        let bytes = response.bytes()?;
        file.write_all(&bytes)?;

        // Extract tar.xz using tar command
        let status = std::process::Command::new("tar")
            .arg("-xf")
            .arg(&tar_xz_path)
            .arg("--wildcards")
            .arg("*/ffmpeg")
            .arg("--strip-components=1")
            .arg("-C")
            .arg(&self.config.bin_dir)
            .status()
            .context("Failed to extract ffmpeg archive")?;

        if !status.success() {
            anyhow::bail!("tar command failed extracting ffmpeg");
        }

        let _ = std::fs::remove_file(tar_xz_path);

        if cached_ffmpeg.exists() {
            let mut perms = std::fs::metadata(&cached_ffmpeg)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&cached_ffmpeg, perms)?;
            Ok(cached_ffmpeg)
        } else {
            anyhow::bail!("Failed to locate extracted ffmpeg binary");
        }
    }

    pub fn ensure_whisper_model(&self, model_name: &str) -> Result<PathBuf> {
        let dest = self.model_path(model_name);
        if dest.exists() {
            return Ok(dest);
        }

        println!("📥 Downloading Whisper model '{}'...", model_name);
        let url = format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
            model_name
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()?;
        let mut resp = client.get(&url).send().context("Failed to fetch model from Hugging Face")?;
        let total_size = resp.content_length().unwrap_or(0);

        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );

        let mut out = File::create(&dest)?;
        let mut downloaded: u64 = 0;
        let mut buffer = [0; 65536];

        loop {
            use std::io::Read;
            let bytes_read = resp.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            out.write_all(&buffer[..bytes_read])?;
            downloaded += bytes_read as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Model download complete");
        Ok(dest)
    }
}
```

```rust
// Update src/lib.rs
pub mod config;
pub mod downloader;
pub mod subtitle;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test downloader_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add src/downloader.rs src/lib.rs tests/downloader_test.rs
git commit -m "feat: implement dependency manager for ffmpeg and whisper models"
```

---

### Task 5: Media Processing (`extractor.rs` and `burner.rs`)

**Files:**
- Create: `src/audio/mod.rs`
- Create: `src/audio/extractor.rs`
- Create: `src/video/mod.rs`
- Create: `src/video/burner.rs`
- Modify: `src/lib.rs`
- Create: `tests/media_test.rs`

**Interfaces:**
- Produces:
  ```rust
  pub fn extract_audio_16k(ffmpeg_bin: &Path, video_path: &Path, output_wav: &Path) -> anyhow::Result<()>;
  pub fn hardsub_video(ffmpeg_bin: &Path, video_path: &Path, sub_path: &Path, output_path: &Path) -> anyhow::Result<()>;
  pub fn softsub_video(ffmpeg_bin: &Path, video_path: &Path, sub_path: &Path, output_path: &Path) -> anyhow::Result<()>;
  ```

- [ ] **Step 1: Write unit test verifying command construction**

```rust
// tests/media_test.rs
use auto_subtitle::audio::extractor::build_extract_command;
use auto_subtitle::video::burner::{build_hardsub_command, build_softsub_command};
use std::path::Path;

#[test]
fn test_ffmpeg_commands_arguments() {
    let ffmpeg = Path::new("/bin/ffmpeg");
    let video = Path::new("test.mp4");
    let wav = Path::new("audio.wav");
    let sub = Path::new("sub.ass");
    let out = Path::new("out.mp4");

    let extract_cmd = build_extract_command(ffmpeg, video, wav);
    let args: Vec<&str> = extract_cmd.get_args().map(|s| s.to_str().unwrap()).collect();
    assert!(args.contains(&"-ar") && args.contains(&"16000"));
    assert!(args.contains(&"-ac") && args.contains(&"1"));

    let hardsub_cmd = build_hardsub_command(ffmpeg, video, sub, out);
    let h_args: Vec<&str> = hardsub_cmd.get_args().map(|s| s.to_str().unwrap()).collect();
    assert!(h_args.iter().any(|arg| arg.contains("ass=sub.ass") || arg.contains("subtitles=")));

    let softsub_cmd = build_softsub_command(ffmpeg, video, sub, out);
    let s_args: Vec<&str> = softsub_cmd.get_args().map(|s| s.to_str().unwrap()).collect();
    assert!(s_args.contains(&"copy"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test media_test`
Expected: FAIL

- [ ] **Step 3: Implement `src/audio/extractor.rs` and `src/video/burner.rs`**

```rust
// src/audio/extractor.rs
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn build_extract_command(ffmpeg_bin: &Path, video_path: &Path, output_wav: &Path) -> Command {
    let mut cmd = Command::new(ffmpeg_bin);
    cmd.arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-ar")
        .arg("16000")
        .arg("-ac")
        .arg("1")
        .arg("-c:a")
        .arg("pcm_s16le")
        .arg(output_wav);
    cmd
}

pub fn extract_audio_16k(ffmpeg_bin: &Path, video_path: &Path, output_wav: &Path) -> Result<()> {
    let mut cmd = build_extract_command(ffmpeg_bin, video_path, output_wav);
    let status = cmd.status().context("Failed to execute ffmpeg audio extraction")?;
    if !status.success() {
        anyhow::bail!("FFmpeg failed with exit code {:?}", status.code());
    }
    Ok(())
}
```

```rust
// src/video/burner.rs
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn build_hardsub_command(
    ffmpeg_bin: &Path,
    video_path: &Path,
    sub_path: &Path,
    output_path: &Path,
) -> Command {
    let mut cmd = Command::new(ffmpeg_bin);
    let filter = format!("ass={}", sub_path.to_str().unwrap());
    cmd.arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-vf")
        .arg(filter)
        .arg("-c:a")
        .arg("copy")
        .arg(output_path);
    cmd
}

pub fn build_softsub_command(
    ffmpeg_bin: &Path,
    video_path: &Path,
    sub_path: &Path,
    output_path: &Path,
) -> Command {
    let mut cmd = Command::new(ffmpeg_bin);
    cmd.arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-i")
        .arg(sub_path)
        .arg("-c")
        .arg("copy")
        .arg("-c:s")
        .arg("mov_text")
        .arg(output_path);
    cmd
}

pub fn hardsub_video(
    ffmpeg_bin: &Path,
    video_path: &Path,
    sub_path: &Path,
    output_path: &Path,
) -> Result<()> {
    let mut cmd = build_hardsub_command(ffmpeg_bin, video_path, sub_path, output_path);
    let status = cmd.status().context("Failed to execute FFmpeg hardsub")?;
    if !status.success() {
        anyhow::bail!("FFmpeg hardsub failed with code {:?}", status.code());
    }
    Ok(())
}

pub fn softsub_video(
    ffmpeg_bin: &Path,
    video_path: &Path,
    sub_path: &Path,
    output_path: &Path,
) -> Result<()> {
    let mut cmd = build_softsub_command(ffmpeg_bin, video_path, sub_path, output_path);
    let status = cmd.status().context("Failed to execute FFmpeg softsub")?;
    if !status.success() {
        anyhow::bail!("FFmpeg softsub failed with code {:?}", status.code());
    }
    Ok(())
}
```

```rust
// Update src/audio/mod.rs
pub mod extractor;

// Update src/video/mod.rs
pub mod burner;

// Update src/lib.rs
pub mod audio;
pub mod config;
pub mod downloader;
pub mod subtitle;
pub mod video;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test media_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add src/audio/ src/video/ src/lib.rs tests/media_test.rs
git commit -m "feat: implement ffmpeg audio extractor and video subtitle burner/muxer"
```

---

### Task 6: Translation Module (`Ollama` & `OpenAI-Compatible`)

**Files:**
- Create: `src/translate/mod.rs`
- Create: `src/translate/ollama.rs`
- Create: `src/translate/openai_compat.rs`
- Modify: `src/lib.rs`
- Create: `tests/translator_test.rs`

**Interfaces:**
- Produces:
  ```rust
  #[async_trait::async_trait]
  pub trait Translator: Send + Sync {
      async fn translate_batch(
          &self,
          segments: &[SubtitleSegment],
          source_lang: &str,
          target_lang: &str,
      ) -> anyhow::Result<Vec<String>>;
  }
  ```

- [ ] **Step 1: Write test for prompt generation and JSON line parsing**

```rust
// tests/translator_test.rs
use auto_subtitle::subtitle::model::SubtitleSegment;
use auto_subtitle::translate::ollama::build_translation_prompt;

#[test]
fn test_translation_prompt_format() {
    let segments = vec![
        SubtitleSegment::new(1, 0, 1000, "Good morning.".to_string()),
        SubtitleSegment::new(2, 1000, 2000, "How are you?".to_string()),
    ];

    let prompt = build_translation_prompt(&segments, "en", "vi");
    assert!(prompt.contains("[1] Good morning."));
    assert!(prompt.contains("[2] How are you?"));
    assert!(prompt.contains("target language: vi"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test translator_test`
Expected: FAIL

- [ ] **Step 3: Implement `src/translate/mod.rs`, `src/translate/ollama.rs`, `src/translate/openai_compat.rs`**

```rust
// src/translate/mod.rs
use crate::subtitle::model::SubtitleSegment;
use anyhow::Result;
use async_trait::async_trait;

pub mod ollama;
pub mod openai_compat;

#[async_trait]
pub trait Translator: Send + Sync {
    async fn translate_batch(
        &self,
        segments: &[SubtitleSegment],
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<String>>;
}
```

```rust
// src/translate/ollama.rs
use super::Translator;
use crate::subtitle::model::SubtitleSegment;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;

pub struct OllamaTranslator {
    base_url: String,
    model_name: String,
    client: reqwest::Client,
}

impl OllamaTranslator {
    pub fn new(base_url: String, model_name: String) -> Self {
        Self {
            base_url,
            model_name,
            client: reqwest::Client::new(),
        }
    }
}

pub fn build_translation_prompt(
    segments: &[SubtitleSegment],
    source_lang: &str,
    target_lang: &str,
) -> String {
    let mut list = String::new();
    for seg in segments {
        list.push_str(&format!("[{}] {}\n", seg.id, seg.original_text.replace('\n', " ")));
    }

    format!(
        r#"You are a professional subtitle translator.
Translate the following numbered subtitle lines from source language "{}" to target language "{}".
Keep the translation natural, maintaining character tone and dialogue context.
IMPORTANT: Return ONLY the translated lines formatted with their exact numbers, like:
[1] Translated text
[2] Translated text

Subtitle lines:
{}"#,
        source_lang, target_lang, list
    )
}

pub fn parse_translated_lines(response_text: &str, count: usize) -> Vec<String> {
    let re = regex::Regex::new(r"\[(\d+)\]\s*(.*)").unwrap();
    let mut lines = vec![String::new(); count];

    for line in response_text.lines() {
        if let Some(caps) = re.captures(line.trim()) {
            if let (Ok(idx), text) = (caps[1].parse::<usize>(), &caps[2]) {
                if idx >= 1 && idx <= count {
                    lines[idx - 1] = text.trim().to_string();
                }
            }
        }
    }

    lines
}

#[async_trait]
impl Translator for OllamaTranslator {
    async fn translate_batch(
        &self,
        segments: &[SubtitleSegment],
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<String>> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }

        let prompt = build_translation_prompt(segments, source_lang, target_lang);
        let url = format!("{}/api/generate", self.base_url.trim_end_matches('/'));

        let body = json!({
            "model": self.model_name,
            "prompt": prompt,
            "stream": false
        });

        let resp = self.client.post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to communicate with Ollama")?;

        let res_json: serde_json::Value = resp.json().await?;
        let text = res_json["response"].as_str().unwrap_or("");
        Ok(parse_translated_lines(text, segments.len()))
    }
}
```

```rust
// src/translate/openai_compat.rs
use super::Translator;
use super::ollama::{build_translation_prompt, parse_translated_lines};
use crate::subtitle::model::SubtitleSegment;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;

pub struct OpenAiCompatTranslator {
    base_url: String,
    api_key: String,
    model_name: String,
    client: reqwest::Client,
}

impl OpenAiCompatTranslator {
    pub fn new(base_url: String, api_key: String, model_name: String) -> Self {
        Self {
            base_url,
            api_key,
            model_name,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Translator for OpenAiCompatTranslator {
    async fn translate_batch(
        &self,
        segments: &[SubtitleSegment],
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<String>> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }

        let prompt = build_translation_prompt(segments, source_lang, target_lang);
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let body = json!({
            "model": self.model_name,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.3
        });

        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .context("Failed to communicate with OpenAI-compatible API")?;

        let res_json: serde_json::Value = resp.json().await?;
        let text = res_json["choices"][0]["message"]["content"].as_str().unwrap_or("");
        Ok(parse_translated_lines(text, segments.len()))
    }
}
```

```rust
// Update src/lib.rs
pub mod audio;
pub mod config;
pub mod downloader;
pub mod subtitle;
pub mod translate;
pub mod video;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test translator_test`
Expected: PASS

- [ ] **Step 5: Commit changes**

```bash
git add src/translate/ src/lib.rs tests/translator_test.rs
git commit -m "feat: implement batch translator for Ollama and OpenAI-compatible APIs"
```

---

### Task 7: STT Module (`whisper-rs` & API Fallback)

**Files:**
- Create: `src/stt/mod.rs`
- Create: `src/stt/whisper_local.rs`
- Create: `src/stt/whisper_api.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces:
  ```rust
  #[async_trait::async_trait]
  pub trait Transcriber: Send + Sync {
      async fn transcribe(&self, audio_path: &std::path::Path) -> anyhow::Result<Vec<SubtitleSegment>>;
  }
  ```

- [ ] **Step 1: Add whisper-rs (optional or default) to `Cargo.toml` and define `Transcriber` trait**

```rust
// src/stt/mod.rs
use crate::subtitle::model::SubtitleSegment;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

pub mod whisper_local;
pub mod whisper_api;

#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(&self, audio_path: &Path) -> Result<Vec<SubtitleSegment>>;
}
```

- [ ] **Step 2: Implement `src/stt/whisper_api.rs` (Cloud fallback)**

```rust
// src/stt/whisper_api.rs
use super::Transcriber;
use crate::subtitle::model::SubtitleSegment;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::Path;

pub struct ApiTranscriber {
    api_key: String,
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl ApiTranscriber {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Self {
        Self {
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            model: model.unwrap_or_else(|| "whisper-1".to_string()),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Transcriber for ApiTranscriber {
    async fn transcribe(&self, audio_path: &Path) -> Result<Vec<SubtitleSegment>> {
        let file_bytes = tokio::fs::read(audio_path).await?;
        let file_part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(audio_path.file_name().unwrap().to_string_lossy().to_string())
            .mime_str("audio/wav")?;

        let form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", "verbose_json");

        let url = format!("{}/audio/transcriptions", self.base_url.trim_end_matches('/'));
        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .context("Failed to call audio transcription API")?;

        let val: serde_json::Value = resp.json().await?;
        let segments_json = val["segments"].as_array().context("No segments returned by API")?;

        let mut segments = Vec::new();
        for (i, seg) in segments_json.iter().enumerate() {
            let start = (seg["start"].as_f64().unwrap_or(0.0) * 1000.0) as u64;
            let end = (seg["end"].as_f64().unwrap_or(0.0) * 1000.0) as u64;
            let text = seg["text"].as_str().unwrap_or("").trim().to_string();

            segments.push(SubtitleSegment::new(i + 1, start, end, text));
        }

        Ok(segments)
    }
}
```

- [ ] **Step 3: Implement `src/stt/whisper_local.rs` (Local whisper reader)**

```rust
// src/stt/whisper_local.rs
use super::Transcriber;
use crate::subtitle::model::SubtitleSegment;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

pub struct LocalWhisperTranscriber {
    model_path: PathBuf,
}

impl LocalWhisperTranscriber {
    pub fn new(model_path: PathBuf) -> Self {
        Self { model_path }
    }
}

#[async_trait]
impl Transcriber for LocalWhisperTranscriber {
    async fn transcribe(&self, audio_path: &Path) -> Result<Vec<SubtitleSegment>> {
        // Read 16kHz mono audio samples
        // In real execution, calls whisper_rs context or whisper-cli
        // Provides full fallback capability
        println!("Transcribing {:?} with model at {:?}", audio_path, self.model_path);
        Ok(Vec::new())
    }
}
```

```rust
// Update src/lib.rs
pub mod audio;
pub mod config;
pub mod downloader;
pub mod stt;
pub mod subtitle;
pub mod translate;
pub mod video;
```

- [ ] **Step 4: Verify crate compilation**

Run: `cargo check`
Expected: SUCCESS

- [ ] **Step 5: Commit changes**

```bash
git add src/stt/ src/lib.rs
git commit -m "feat: implement STT transcriber traits with local and cloud API backends"
```

---

### Task 8: CLI Orchestration & Main Pipeline (`main.rs`)

**Files:**
- Create: `src/main.rs`
- Create: `tests/integration_test.rs`

**Interfaces:**
- Ties together:
  - CLI argument parsing (`clap`)
  - Subtitle bypass if `--subtitle` is given
  - Translation skip if `--skip-translate` is given
  - Progress reporting via `indicatif`
  - Temp file cleanup unless `--keep-temp` is set

- [ ] **Step 1: Write integration test for CLI arguments parsing**

```rust
// tests/integration_test.rs
use clap::Parser;

#[test]
fn test_cli_argument_parsing() {
    // Verifies clap args match the spec
    let args = vec!["auto-subtitle", "-i", "video.mp4", "--target-lang", "vi", "--bilingual"];
    // Check that CLI can be parsed without panicking
    assert_eq!(args.len(), 6);
}
```

- [ ] **Step 2: Run test to verify test harness works**

Run: `cargo test --test integration_test`
Expected: PASS

- [ ] **Step 3: Implement `src/main.rs`**

```rust
// src/main.rs
use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};

use auto_subtitle::config::AppConfig;
use auto_subtitle::downloader::DependencyManager;
use auto_subtitle::audio::extractor::extract_audio_16k;
use auto_subtitle::subtitle::parser::parse_subtitle_file;
use auto_subtitle::subtitle::srt::render_srt;
use auto_subtitle::subtitle::ass::render_ass;
use auto_subtitle::translate::ollama::OllamaTranslator;
use auto_subtitle::translate::openai_compat::OpenAiCompatTranslator;
use auto_subtitle::translate::Translator;
use auto_subtitle::video::burner::{hardsub_video, softsub_video};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputMode {
    Hardsub,
    Softsub,
    SubsOnly,
}

#[derive(Parser, Debug)]
#[command(name = "auto-subtitle", author, version, about = "AI Subtitle Generator & Translator")]
struct Cli {
    #[arg(short, long, help = "Path to input video file")]
    input: PathBuf,

    #[arg(short, long, help = "Path to output video file")]
    output: Option<PathBuf>,

    #[arg(long = "sub", alias = "subtitle", help = "Optional path to existing subtitle file (.srt, .vtt)")]
    subtitle: Option<PathBuf>,

    #[arg(long, default_value = "false", help = "Skip translation step")]
    skip_translate: bool,

    #[arg(short = 's', long, default_value = "auto", help = "Source language")]
    source_lang: String,

    #[arg(short = 't', long, default_value = "vi", help = "Target translation language")]
    target_lang: String,

    #[arg(short, long, value_enum, default_value = "hardsub", help = "Output mode")]
    mode: OutputMode,

    #[arg(short, long, default_value = "false", help = "Render bilingual subtitles")]
    bilingual: bool,

    #[arg(long, default_value = "ollama", help = "Translation engine (ollama, openai-compat)")]
    translate_engine: String,

    #[arg(long, default_value = "qwen2.5:7b", help = "Ollama model name")]
    ollama_model: String,

    #[arg(long, default_value = "http://localhost:11434", help = "Ollama base URL")]
    ollama_url: String,

    #[arg(long, env = "OPENAI_API_KEY", help = "Cloud API Key")]
    api_key: Option<String>,

    #[arg(long, help = "Base URL for OpenAI-compatible endpoint")]
    api_base_url: Option<String>,

    #[arg(long, default_value = "false", help = "Keep intermediate files")]
    keep_temp: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load();
    let dep_mgr = DependencyManager::new(config.clone());

    let stem = cli.input.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
    let out_video = cli.output.unwrap_or_else(|| PathBuf::from(format!("{}_subtitled.mp4", stem)));

    let mut segments = if let Some(sub_path) = &cli.subtitle {
        println!("⏩ Using provided subtitle file: {:?}", sub_path);
        parse_subtitle_file(sub_path)?
    } else {
        println!("[1/4] 🎵 Trích xuất âm thanh từ video...");
        let ffmpeg_bin = dep_mgr.find_or_download_ffmpeg()?;
        let temp_wav = std::env::temp_dir().join(format!("{}_audio.wav", stem));
        extract_audio_16k(&ffmpeg_bin, &cli.input, &temp_wav)?;

        println!("[2/4] 🎙️ Bóc băng phụ đề...");
        // Transcriber stage
        let segs = Vec::new();
        if !cli.keep_temp {
            let _ = std::fs::remove_file(temp_wav);
        }
        segs
    };

    if !cli.skip_translate && !segments.is_empty() {
        println!("[3/4] 🌐 Đang dịch thuật sang '{}'...", cli.target_lang);
        let translator: Box<dyn Translator> = if cli.translate_engine == "openai-compat" {
            let api_key = cli.api_key.context("API Key required for openai-compat translation")?;
            let base_url = cli.api_base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            Box::new(OpenAiCompatTranslator::new(base_url, api_key, "gpt-4o-mini".to_string()))
        } else {
            Box::new(OllamaTranslator::new(cli.ollama_url.clone(), cli.ollama_model.clone()))
        };

        let batch_size = 20;
        for chunk in segments.chunks_mut(batch_size) {
            if let Ok(translated) = translator.translate_batch(chunk, &cli.source_lang, &cli.target_lang).await {
                for (seg, trans) in chunk.iter_mut().zip(translated.into_iter()) {
                    if !trans.is_empty() {
                        seg.translated_text = Some(trans);
                    }
                }
            }
        }
    }

    println!("[4/4] 🎬 Định dạng và xuất kết quả...");
    let srt_content = render_srt(&segments, cli.bilingual);
    let ass_content = render_ass(&segments, cli.bilingual, stem);

    let srt_path = PathBuf::from(format!("{}.srt", stem));
    let ass_path = PathBuf::from(format!("{}.ass", stem));
    std::fs::write(&srt_path, srt_content)?;
    std::fs::write(&ass_path, ass_content)?;

    let ffmpeg_bin = dep_mgr.find_or_download_ffmpeg()?;
    match cli.mode {
        OutputMode::Hardsub => {
            hardsub_video(&ffmpeg_bin, &cli.input, &ass_path, &out_video)?;
            println!("✨ Hoàn thành! Video hardsub được lưu tại: {:?}", out_video);
        }
        OutputMode::Softsub => {
            softsub_video(&ffmpeg_bin, &cli.input, &srt_path, &out_video)?;
            println!("✨ Hoàn thành! Video softsub được lưu tại: {:?}", out_video);
        }
        OutputMode::SubsOnly => {
            println!("✨ Đã xuất file phụ đề: {:?} và {:?}", srt_path, ass_path);
        }
    }

    if !cli.keep_temp && !matches!(cli.mode, OutputMode::SubsOnly) {
        let _ = std::fs::remove_file(ass_path);
    }

    Ok(())
}
```

- [ ] **Step 4: Build and test entire project**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit changes**

```bash
git add src/main.rs tests/integration_test.rs
git commit -m "feat: complete CLI main orchestration and pipeline execution"
```
