# Auto-Subtitle Rust CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI that turns a video (or an existing subtitle file) into translated subtitles and outputs `.srt`/`.ass` files, a hardsubbed video, or a softsubbed video.

**Spec:** `docs/superpowers/specs/2026-10-31-auto-subtitle-rust-cli-design.md`
**Review:** `docs/superpowers/reviews/2026-10-31-auto-subtitle-rust-cli-plan-review.md` (revision 2 — fixes C1–C4, I1–I6)

**Architecture:** Library crate (`src/lib.rs`) holds all logic behind two traits — `Transcriber` and `Translator` — plus pure subtitle parse/render modules and FFmpeg command builders. `src/pipeline.rs` orchestrates using trait objects so it can be tested end-to-end with mocks. `src/main.rs` only builds concrete dependencies (downloader, Whisper, Ollama/OpenAI) and calls the pipeline.

**Tech Stack:** Rust 2021, `clap` 4, `tokio`, `reqwest` 0.12 (async, `json`/`stream`/`multipart`/`rustls-tls`), `serde`/`serde_json`, `whisper-rs` 0.14 (+ optional `cuda` feature), `hound`, `indicatif`, `anyhow`, `tempfile`, `which`, `regex`; tests: `wiremock`.

## Global Constraints

- Rust 2021 edition; toolchain on machine: `rustc 1.99.1`.
- Errors: `anyhow::Result` with `.context(...)` on every I/O / process / HTTP boundary. No `unwrap()` on user-controlled data (paths, HTTP bodies, LLM output).
- Cache root: `dirs::cache_dir()/auto-subtitle/` with `models/` and `bin/`. Tests MUST use a `tempfile::TempDir` base, never the real cache.
- **Never write into or over user input files.** Intermediate files live in a `tempfile` work dir, deleted unless `--keep-temp`.
- All HTTP responses checked with `error_for_status()`; error messages include the response body (truncated to 500 chars).
- FFmpeg invoked with `-hide_banner -loglevel error -y`; on failure, error includes last 20 lines of stderr.
- Every task ends green on: `cargo test` and `cargo clippy --all-targets -- -D warnings`.
- Supported platform for auto-download of FFmpeg: Linux x86_64. Other platforms: clear error asking user to install FFmpeg.

---

### Task 0: Toolchain Preflight

`whisper-rs` builds `whisper.cpp` from source: needs `cmake`, a C/C++ compiler, and `libclang` (bindgen). The CUDA build additionally needs the CUDA toolkit (`nvcc`). On this machine `cmake`, `clang`, `nvcc`, `ollama` are currently **missing**.

**Files:**
- Create: `README.md` (Prerequisites + Usage sections)

- [ ] **Step 1: Install build prerequisites (user runs, needs sudo)**

```bash
sudo apt install -y build-essential cmake clang libclang-dev pkg-config
# Optional GPU build:
sudo apt install -y nvidia-cuda-toolkit   # provides nvcc
# Optional local translation:
curl -fsSL https://ollama.com/install.sh | sh && ollama pull qwen2.5:7b
```

- [ ] **Step 2: Verify**

Run: `cmake --version && clang --version && cargo --version`
Expected: all three print versions. (`nvcc --version` only required for `--features cuda`.)

- [ ] **Step 3: Write `README.md`** with: prerequisites above, build commands (`cargo build --release`, `cargo build --release --features cuda`), and the usage examples from spec §4.

> If the user cannot install `cmake`/`clang`, Tasks 1–6 and 8 can still proceed; Task 7's local engine is the only part blocked.

---

### Task 1: Scaffolding, Config, Data Model & Sanitization

**Files:**
- Create: `Cargo.toml`, `src/lib.rs`, `src/config.rs`, `src/subtitle/mod.rs`, `src/subtitle/model.rs`
- Test: `tests/model_test.rs`

**Interfaces (Produces):**
```rust
// src/subtitle/model.rs
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SubtitleSegment {
    pub id: usize,               // 1-based, sequential after sanitize
    pub start_ms: u64,
    pub end_ms: u64,
    pub original_text: String,
    pub translated_text: Option<String>,
}
impl SubtitleSegment {
    pub fn new(id: usize, start_ms: u64, end_ms: u64, original_text: impl Into<String>) -> Self;
    pub fn format_timestamp_srt(ms: u64) -> String; // "HH:MM:SS,mmm"
    pub fn format_timestamp_ass(ms: u64) -> String; // "H:MM:SS.cc"
}
/// trim text; drop empty text; sort by start; clip overlap (prev.end = cur.start);
/// drop end <= start; renumber ids 1..=n.
pub fn sanitize_segments(segments: Vec<SubtitleSegment>) -> Vec<SubtitleSegment>;

// src/config.rs
#[derive(Debug, Clone)]
pub struct AppConfig { pub cache_dir: PathBuf, pub models_dir: PathBuf, pub bin_dir: PathBuf }
impl AppConfig {
    pub fn load() -> anyhow::Result<Self>;                 // uses dirs::cache_dir()
    pub fn with_base(base: PathBuf) -> anyhow::Result<Self>; // creates models/ and bin/
}
```

- [ ] **Step 1: Create `Cargo.toml`** (whisper-rs/hound are added in Task 7 to keep early builds fast)

```toml
[package]
name = "auto-subtitle"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4.5", features = ["derive", "env"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "multipart", "rustls-tls"] }
futures-util = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
indicatif = "0.17"
anyhow = "1"
async-trait = "0.1"
dirs = "5"
regex = "1"
tempfile = "3.20"
which = "6"

[dev-dependencies]
wiremock = "0.6"
```

- [ ] **Step 2: Write failing tests** `tests/model_test.rs`

```rust
use auto_subtitle::subtitle::model::{sanitize_segments, SubtitleSegment as S};
use auto_subtitle::config::AppConfig;

#[test]
fn srt_timestamps() {
    assert_eq!(S::format_timestamp_srt(0), "00:00:00,000");
    assert_eq!(S::format_timestamp_srt(1234), "00:00:01,234");
    assert_eq!(S::format_timestamp_srt(3_661_050), "01:01:01,050");
}

#[test]
fn ass_timestamps() {
    assert_eq!(S::format_timestamp_ass(0), "0:00:00.00");
    assert_eq!(S::format_timestamp_ass(1234), "0:00:01.23");
    assert_eq!(S::format_timestamp_ass(3_661_050), "1:01:01.05");
}

#[test]
fn sanitize_sorts_clips_drops_and_renumbers() {
    let input = vec![
        S::new(9, 5000, 6000, "  third  "),
        S::new(1, 1000, 3000, "first"),
        S::new(2, 2500, 4000, "second overlaps first"),
        S::new(3, 7000, 7000, "zero duration"),
        S::new(4, 8000, 9000, "   "),
    ];
    let out = sanitize_segments(input);
    assert_eq!(out.len(), 3);
    assert_eq!((out[0].id, out[0].start_ms, out[0].end_ms), (1, 1000, 2500)); // clipped
    assert_eq!((out[1].id, out[1].start_ms, out[1].end_ms), (2, 2500, 4000));
    assert_eq!(out[2].original_text, "third");                               // trimmed
    assert_eq!(out[2].id, 3);
}

#[test]
fn config_with_base_creates_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = AppConfig::with_base(tmp.path().join("c")).unwrap();
    assert!(cfg.models_dir.is_dir() && cfg.bin_dir.is_dir());
}
```

- [ ] **Step 3: Run** `cargo test --test model_test` → Expected: FAIL (modules missing)

- [ ] **Step 4: Implement** `model.rs`, `config.rs`, `subtitle/mod.rs` (`pub mod model;`), `lib.rs` (`pub mod config; pub mod subtitle;`).

Key implementation (timestamps & sanitize):
```rust
pub fn format_timestamp_srt(ms: u64) -> String {
    let (h, m, s, milli) = (ms / 3_600_000, (ms / 60_000) % 60, (ms / 1000) % 60, ms % 1000);
    format!("{h:02}:{m:02}:{s:02},{milli:03}")
}
pub fn format_timestamp_ass(ms: u64) -> String {
    let (h, m, s, cs) = (ms / 3_600_000, (ms / 60_000) % 60, (ms / 1000) % 60, (ms % 1000) / 10);
    format!("{h}:{m:02}:{s:02}.{cs:02}")
}
pub fn sanitize_segments(mut segs: Vec<SubtitleSegment>) -> Vec<SubtitleSegment> {
    for s in &mut segs {
        s.original_text = s.original_text.trim().to_string();
        if let Some(t) = &mut s.translated_text { *t = t.trim().to_string(); }
    }
    segs.retain(|s| !s.original_text.is_empty());
    segs.sort_by_key(|s| (s.start_ms, s.end_ms));
    for i in 1..segs.len() {
        let next_start = segs[i].start_ms;
        if segs[i - 1].end_ms > next_start { segs[i - 1].end_ms = next_start; }
    }
    segs.retain(|s| s.end_ms > s.start_ms);
    for (i, s) in segs.iter_mut().enumerate() { s.id = i + 1; }
    segs
}
```

- [ ] **Step 5: Run** `cargo test --test model_test && cargo clippy --all-targets -- -D warnings` → Expected: PASS

- [ ] **Step 6: Commit** `git add -A && git commit -m "feat: scaffold crate, config and subtitle model with sanitization"`

---

### Task 2: Subtitle Parser (`.srt`, `.vtt`, `.ass`)

**Files:**
- Create: `src/subtitle/parser.rs`; Modify: `src/subtitle/mod.rs` (`pub mod parser;`)
- Test: `tests/parser_test.rs`, fixtures `tests/fixtures/sample.srt`, `sample.vtt`, `sample.ass`

**Interfaces:**
- Consumes: `SubtitleSegment`, `sanitize_segments` (Task 1)
- Produces:
```rust
pub fn parse_timestamp(ts: &str) -> Option<u64>;   // "HH:MM:SS,mmm" | "HH:MM:SS.mmm" | "MM:SS.mmm" | "H:MM:SS.cc"
pub fn parse_srt(content: &str) -> anyhow::Result<Vec<SubtitleSegment>>;
pub fn parse_vtt(content: &str) -> anyhow::Result<Vec<SubtitleSegment>>;
pub fn parse_ass(content: &str) -> anyhow::Result<Vec<SubtitleSegment>>;
/// Dispatch on extension (case-insensitive). Unknown extension => Err.
/// Result is passed through sanitize_segments. Empty result => Err("no subtitle cues found").
pub fn parse_subtitle_file(path: &Path) -> anyhow::Result<Vec<SubtitleSegment>>;
```

Behavior rules:
- Strip UTF-8 BOM; normalize `\r\n`/`\r` to `\n`; blocks split on blank lines.
- SRT/VTT: the time line is the first line containing `-->`; end timestamp = first whitespace token after `-->` (VTT cue settings like `align:start` ignored); text = following lines joined with `\n`. Blocks without `-->` (VTT header, `NOTE`, `STYLE`) are skipped. Malformed timestamp => `Err` naming the block number.
- ASS: locate `[Events]`, read its `Format:` line to find indexes of `Start`, `End`, `Text`; for each `Dialogue:` line use `splitn(field_count, ',')` (Text may contain commas). Remove override blocks `\{[^}]*\}`, replace `\N`/`\n` with newline, `\h` with space. If the file has two Dialogue lines with identical Start/End (bilingual output of this tool), keep both — sanitize clips nothing since times are equal; this is acceptable for v1.

- [ ] **Step 1: Write failing tests** (`tests/parser_test.rs`)

```rust
use auto_subtitle::subtitle::parser::*;
use std::path::Path;

#[test]
fn srt_multiline_and_crlf() {
    let srt = "1\r\n00:00:01,000 --> 00:00:03,500\r\nHello world!\r\n\r\n2\r\n00:00:04,100 --> 00:00:06,200\r\nLine one\r\nLine two\r\n";
    let s = parse_srt(srt).unwrap();
    assert_eq!(s.len(), 2);
    assert_eq!((s[0].start_ms, s[0].end_ms, s[0].original_text.as_str()), (1000, 3500, "Hello world!"));
    assert_eq!(s[1].original_text, "Line one\nLine two");
}

#[test]
fn vtt_header_note_and_cue_settings() {
    let vtt = "\u{feff}WEBVTT\nKind: captions\n\nNOTE a comment\n\n00:01.500 --> 00:03.000 align:start position:10%\nWebVTT test\n";
    let s = parse_vtt(vtt).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].start_ms, s[0].end_ms), (1500, 3000));
    assert_eq!(s[0].original_text, "WebVTT test");
}

#[test]
fn ass_dialogue_with_commas_and_tags() {
    let ass = "[Script Info]\nTitle: x\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.50,0:00:04.00,Default,,0,0,0,,{\\b1}Hi, there\\Nsecond line\n";
    let s = parse_ass(ass).unwrap();
    assert_eq!((s[0].start_ms, s[0].end_ms), (1500, 4000));
    assert_eq!(s[0].original_text, "Hi, there\nsecond line");
}

#[test]
fn malformed_timestamp_is_error() {
    assert!(parse_srt("1\n00:00:xx,000 --> 00:00:02,000\nBad\n").is_err());
}

#[test]
fn unknown_extension_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("a.txt");
    std::fs::write(&p, "x").unwrap();
    assert!(parse_subtitle_file(&p).is_err());
}

#[test]
fn file_dispatch_by_extension() {
    let s = parse_subtitle_file(Path::new("tests/fixtures/sample.srt")).unwrap();
    assert!(!s.is_empty());
    assert_eq!(s[0].id, 1);
}
```

Create `tests/fixtures/sample.srt` (2 cues), `sample.vtt` (1 cue), `sample.ass` (1 Dialogue) — the same contents as the inline strings above.

- [ ] **Step 2: Run** `cargo test --test parser_test` → FAIL
- [ ] **Step 3: Implement** `parser.rs` per behavior rules. `parse_timestamp` splits on `:`; last part split on `.`/`,`; fractional part right-padded to 3 digits (`"50"` → 500 ms, `"5"` → 500 ms); returns `None` on any non-digit.
- [ ] **Step 4: Run** `cargo test --test parser_test` + clippy → PASS
- [ ] **Step 5: Commit** `git commit -am "feat: parse existing srt/vtt/ass subtitles"` (add fixtures with `git add tests/fixtures`)

---

### Task 3: Subtitle Renderers (`.srt`, `.ass`, bilingual)

**Files:**
- Create: `src/subtitle/srt.rs`, `src/subtitle/ass.rs`; Modify: `src/subtitle/mod.rs`
- Test: `tests/formatter_test.rs`

**Interfaces:**
```rust
// srt.rs
pub fn render_srt(segments: &[SubtitleSegment], bilingual: bool) -> String;
// ass.rs
pub fn escape_ass_text(text: &str) -> String; // '{'->'(' '}'->')' '\\'->'/' then '\n'->"\\N"
pub fn render_ass(segments: &[SubtitleSegment], bilingual: bool, title: &str) -> String;
```

Text selection rule (both formats):
| translated_text | bilingual | Output |
|---|---|---|
| `Some(t)` | false | `t` |
| `Some(t)` | true | original on top, `t` below |
| `None` | any | original |

ASS layout: `PlayResX 1920 / PlayResY 1080`; style `Default` (Arial 64, white, outline 3, `Alignment 2`, `MarginV 40`) for the main line; style `Original` (Arial 44, light grey, `MarginV 120`) for the original line in bilingual mode. Bilingual = two `Dialogue` lines with same times (`Original` layer 0, `Default` layer 1).

- [ ] **Step 1: Write failing tests**

```rust
use auto_subtitle::subtitle::{model::SubtitleSegment, srt::render_srt, ass::{render_ass, escape_ass_text}};

fn seg() -> SubtitleSegment {
    let mut s = SubtitleSegment::new(1, 1500, 4000, "Hello");
    s.translated_text = Some("Xin chào".into());
    s
}

#[test]
fn srt_mono_and_bilingual() {
    let mono = render_srt(&[seg()], false);
    assert_eq!(mono, "1\n00:00:01,500 --> 00:00:04,000\nXin chào\n\n");
    let bi = render_srt(&[seg()], true);
    assert!(bi.contains("Hello\nXin chào"));
}

#[test]
fn srt_untranslated_falls_back_to_original() {
    let s = SubtitleSegment::new(1, 0, 1000, "Only original");
    assert!(render_srt(&[s], true).contains("Only original"));
}

#[test]
fn ass_structure_and_bilingual_lines() {
    let out = render_ass(&[seg()], true, "Video Test");
    assert!(out.contains("[Script Info]") && out.contains("Title: Video Test"));
    assert!(out.contains("Style: Default,") && out.contains("Style: Original,"));
    assert!(out.contains("Dialogue: 0,0:00:01.50,0:00:04.00,Original,,0,0,0,,Hello"));
    assert!(out.contains("Dialogue: 1,0:00:01.50,0:00:04.00,Default,,0,0,0,,Xin chào"));
}

#[test]
fn ass_escapes_override_tags_and_newlines() {
    assert_eq!(escape_ass_text("{\\b1}a\nb"), "(/b1)a\\Nb");
}
```

- [ ] **Step 2: Run** `cargo test --test formatter_test` → FAIL
- [ ] **Step 3: Implement** both renderers (SRT text lines joined with `\n`, each cue followed by one blank line).
- [ ] **Step 4: Run** tests + clippy → PASS
- [ ] **Step 5: Commit** `git commit -am "feat: render srt and ass subtitles with bilingual layout"`

---

### Task 4: Dependency Manager (FFmpeg + Whisper models, async)

**Files:**
- Create: `src/downloader.rs`; Modify: `src/lib.rs`
- Test: `tests/downloader_test.rs`

**Interfaces:**
```rust
pub const WHISPER_MODELS: &[&str] = &["tiny","tiny.en","base","base.en","small","small.en",
                                      "medium","medium.en","large-v3","large-v3-turbo"];
pub struct DependencyManager { config: AppConfig, client: reqwest::Client,
                               model_base_url: String, ffmpeg_url: String }
impl DependencyManager {
    pub fn new(config: AppConfig) -> Self; // HF + johnvansickle default URLs
    pub fn with_urls(config: AppConfig, model_base_url: String, ffmpeg_url: String) -> Self; // tests
    pub fn model_path(&self, name: &str) -> anyhow::Result<PathBuf>; // Err if name not in WHISPER_MODELS
    pub async fn ensure_whisper_model(&self, name: &str) -> anyhow::Result<PathBuf>;
    pub async fn find_or_download_ffmpeg(&self) -> anyhow::Result<PathBuf>;
}
/// Streams url -> dest.part with an indicatif bar, then renames to dest.
/// On any error the .part file is removed and dest is untouched.
pub async fn download_to(client: &reqwest::Client, url: &str, dest: &Path) -> anyhow::Result<()>;
```

Behavior:
- Model URL: `{model_base_url}/ggml-{name}.bin` (default base `https://huggingface.co/ggerganov/whisper.cpp/resolve/main`). Existing non-empty file => return immediately.
- FFmpeg lookup order: `which::which("ffmpeg")` → `bin_dir/ffmpeg` → download. Download only if `cfg!(target_os="linux") && std::env::consts::ARCH == "x86_64"`, else `Err` with install instructions. Download tar.xz into a `TempDir`, extract with `tar -xJf <archive> -C <tmp>`, find the file named `ffmpeg` (walk one level of subdirs), move to `bin_dir/ffmpeg`, `chmod 0o755`, then verify `ffmpeg -version` exits 0.
- Use `response.bytes_stream()` (futures-util) — **no `reqwest::blocking`** (would panic inside the tokio runtime).

- [ ] **Step 1: Write failing tests** (wiremock, no real network)

```rust
use auto_subtitle::{config::AppConfig, downloader::*};
use wiremock::{MockServer, Mock, ResponseTemplate, matchers::path};

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
    Mock::given(path("/ggml-tiny.bin")).respond_with(ResponseTemplate::new(200).set_body_bytes(vec![7u8; 1024]))
        .mount(&server).await;
    let (_t, c) = cfg();
    let m = DependencyManager::with_urls(c.clone(), server.uri(), "unused".into());
    let p = m.ensure_whisper_model("tiny").await.unwrap();
    assert_eq!(std::fs::read(&p).unwrap().len(), 1024);
    assert!(!p.with_extension("bin.part").exists());
}

#[tokio::test]
async fn http_error_leaves_no_file() {
    let server = MockServer::start().await;
    Mock::given(path("/ggml-base.bin")).respond_with(ResponseTemplate::new(404)).mount(&server).await;
    let (_t, c) = cfg();
    let m = DependencyManager::with_urls(c.clone(), server.uri(), "unused".into());
    assert!(m.ensure_whisper_model("base").await.is_err());
    assert!(!c.models_dir.join("ggml-base.bin").exists());
    assert!(!c.models_dir.join("ggml-base.bin.part").exists());
}
```

- [ ] **Step 2: Run** `cargo test --test downloader_test` → FAIL
- [ ] **Step 3: Implement** `downloader.rs` per behavior; add `pub mod downloader;` to `lib.rs`.
- [ ] **Step 4: Run** tests + clippy → PASS
- [ ] **Step 5: Commit** `git commit -am "feat: async dependency manager for ffmpeg and whisper models"` (real-network FFmpeg/model download is verified in the Task 8 smoke test)

---

### Task 5: Media Processing (audio extraction, hardsub, softsub)

**Files:**
- Create: `src/media.rs` (single focused module: command builders + runner); Modify: `src/lib.rs`
- Test: `tests/media_test.rs`

**Interfaces:**
```rust
/// 16 kHz mono s16le WAV for local Whisper.
pub fn build_extract_wav_command(ffmpeg: &Path, video: &Path, out_wav: &Path) -> Command;
/// 16 kHz mono 32 kbps MP3 for cloud STT (keeps uploads under 25 MB for ~100 min).
pub fn build_extract_mp3_command(ffmpeg: &Path, video: &Path, out_mp3: &Path) -> Command;
/// Runs ffmpeg with current_dir = ass_path.parent() and `-vf ass=<file_name>`.
/// Err if ass file name is not [A-Za-z0-9._-]+ (we always use "subtitle.ass").
/// video/output are made absolute with std::path::absolute.
pub fn build_hardsub_command(ffmpeg: &Path, video: &Path, ass_path: &Path, output: &Path) -> anyhow::Result<Command>;
/// Subtitle codec by output extension: mp4|m4v|mov -> mov_text, mkv -> srt, else Err.
pub fn build_softsub_command(ffmpeg: &Path, video: &Path, srt_path: &Path, output: &Path) -> anyhow::Result<Command>;
/// Runs the command; on non-zero exit returns Err containing the last 20 stderr lines.
pub fn run_ffmpeg(cmd: Command, what: &str) -> anyhow::Result<()>;
```

Exact argument lists:
- extract WAV: `-hide_banner -loglevel error -y -i <video> -vn -ar 16000 -ac 1 -c:a pcm_s16le <out>`
- extract MP3: `-hide_banner -loglevel error -y -i <video> -vn -ar 16000 -ac 1 -c:a libmp3lame -b:a 32k <out>`
- hardsub: `-hide_banner -loglevel error -y -i <abs video> -map 0:v:0 -map 0:a? -vf ass=subtitle.ass -c:v libx264 -preset medium -crf 20 -c:a copy <abs output>`
- softsub: `-hide_banner -loglevel error -y -i <video> -i <srt> -map 0:v -map 0:a? -map 1:0 -c:v copy -c:a copy -c:s <codec> <output>`

- [ ] **Step 1: Write failing tests**

```rust
use auto_subtitle::media::*;
use std::path::Path;

fn args(c: &std::process::Command) -> Vec<String> {
    c.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
}

#[test]
fn wav_extract_args() {
    let a = args(&build_extract_wav_command(Path::new("ffmpeg"), Path::new("in.mp4"), Path::new("a.wav")));
    assert!(a.windows(2).any(|w| w == ["-ar", "16000"]));
    assert!(a.windows(2).any(|w| w == ["-ac", "1"]));
    assert!(a.windows(2).any(|w| w == ["-c:a", "pcm_s16le"]));
}

#[test]
fn hardsub_uses_workdir_and_safe_filter() {
    let tmp = tempfile::tempdir().unwrap();
    let ass = tmp.path().join("subtitle.ass");
    let cmd = build_hardsub_command(Path::new("ffmpeg"), Path::new("My Clip: 1.mp4"), &ass, Path::new("out.mp4")).unwrap();
    assert_eq!(cmd.get_current_dir(), Some(tmp.path()));
    let a = args(&cmd);
    assert!(a.windows(2).any(|w| w == ["-vf", "ass=subtitle.ass"]));
    assert!(a.iter().any(|x| x.ends_with("My Clip: 1.mp4") && Path::new(x).is_absolute()));
}

#[test]
fn hardsub_rejects_unsafe_ass_name() {
    assert!(build_hardsub_command(Path::new("ffmpeg"), Path::new("a.mp4"), Path::new("/tmp/a b:c.ass"), Path::new("o.mp4")).is_err());
}

#[test]
fn softsub_codec_by_container() {
    let mp4 = args(&build_softsub_command(Path::new("ffmpeg"), Path::new("a.mp4"), Path::new("s.srt"), Path::new("o.mp4")).unwrap());
    assert!(mp4.windows(2).any(|w| w == ["-c:s", "mov_text"]));
    let mkv = args(&build_softsub_command(Path::new("ffmpeg"), Path::new("a.mkv"), Path::new("s.srt"), Path::new("o.mkv")).unwrap());
    assert!(mkv.windows(2).any(|w| w == ["-c:s", "srt"]));
    assert!(build_softsub_command(Path::new("ffmpeg"), Path::new("a.mp4"), Path::new("s.srt"), Path::new("o.avi")).is_err());
}
```

- [ ] **Step 2: Run** `cargo test --test media_test` → FAIL
- [ ] **Step 3: Implement** `media.rs`; `run_ffmpeg` uses `cmd.output()` and formats stderr tail.
- [ ] **Step 4: Run** tests + clippy → PASS
- [ ] **Step 5: Commit** `git commit -am "feat: ffmpeg command builders for extraction, hardsub and softsub"`

> Note: the module layout differs from spec §2.1 (`audio/extractor.rs` + `video/burner.rs` merged into `media.rs`) because both are thin FFmpeg command builders sharing `run_ffmpeg`; boundaries and behavior are unchanged.

---

### Task 6: Translation (Ollama + OpenAI-compatible)

**Files:**
- Create: `src/translate/mod.rs`, `src/translate/prompt.rs`, `src/translate/ollama.rs`, `src/translate/openai_compat.rs`; Modify: `src/lib.rs`
- Test: `tests/translator_test.rs`

**Interfaces:**
```rust
// mod.rs
#[async_trait::async_trait]
pub trait Translator: Send + Sync {
    /// MUST return exactly segments.len() strings, in order, or Err.
    async fn translate_batch(&self, segments: &[SubtitleSegment], source_lang: &str, target_lang: &str)
        -> anyhow::Result<Vec<String>>;
}
// prompt.rs
/// Lines numbered LOCALLY 1..=n (never seg.id). Newlines inside a cue become " / ".
/// source_lang "auto" => "Detect the source language automatically."
pub fn build_translation_prompt(segments: &[SubtitleSegment], source_lang: &str, target_lang: &str) -> String;
/// Parses "[k] text" lines; Err if any k in 1..=count missing; ignores extras/out-of-range;
/// " / " converted back to "\n".
pub fn parse_translated_lines(response: &str, count: usize) -> anyhow::Result<Vec<String>>;
// ollama.rs
pub struct OllamaTranslator { /* base_url, model, client */ }
impl OllamaTranslator {
    pub fn new(base_url: String, model: String) -> Self;
    /// GET {base}/api/tags. Unreachable => Err("Không kết nối được với Ollama ... chạy 'ollama serve' hoặc dùng --translate-engine openai-compat").
    /// Model missing from tags list (match "name" exact, or "name:latest" when no tag given) => Err("... chạy 'ollama pull <model>'").
    pub async fn health_check(&self) -> anyhow::Result<()>;
}
// POST {base}/api/chat {model, stream:false, options:{temperature:0.2}, messages:[{role:"user",content:prompt}]}
// reads .message.content
// openai_compat.rs
pub struct OpenAiCompatTranslator { /* base_url, api_key, model, client */ }
impl OpenAiCompatTranslator { pub fn new(base_url: String, api_key: String, model: String) -> Self; }
// POST {base}/chat/completions, Bearer auth, temperature 0.2, reads .choices[0].message.content
```

- [ ] **Step 1: Write failing tests**

```rust
use auto_subtitle::subtitle::model::SubtitleSegment as S;
use auto_subtitle::translate::{prompt::*, ollama::OllamaTranslator, openai_compat::OpenAiCompatTranslator, Translator};
use wiremock::{MockServer, Mock, ResponseTemplate, matchers::{method, path, header}};
use serde_json::json;

fn batch() -> Vec<S> { vec![S::new(21, 0, 1000, "Good morning."), S::new(22, 1000, 2000, "How are\nyou?")] }

#[test]
fn prompt_uses_local_numbering() {
    let p = build_translation_prompt(&batch(), "en", "vi");
    assert!(p.contains("[1] Good morning.") && p.contains("[2] How are / you?"));
    assert!(!p.contains("[21]"));
}

#[test]
fn parse_ok_and_missing_line_errors() {
    let ok = parse_translated_lines("Here:\n[1] Chào buổi sáng.\n[2] Bạn / khỏe không?\n", 2).unwrap();
    assert_eq!(ok, vec!["Chào buổi sáng.", "Bạn\nkhỏe không?"]);
    assert!(parse_translated_lines("[1] only one", 2).is_err());
}

#[tokio::test]
async fn ollama_translate_and_health() {
    let s = MockServer::start().await;
    Mock::given(method("GET")).and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models":[{"name":"qwen2.5:7b"}]}))).mount(&s).await;
    Mock::given(method("POST")).and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"message":{"content":"[1] A\n[2] B"}}))).mount(&s).await;
    let t = OllamaTranslator::new(s.uri(), "qwen2.5:7b".into());
    t.health_check().await.unwrap();
    assert_eq!(t.translate_batch(&batch(), "en", "vi").await.unwrap(), vec!["A", "B"]);
    assert!(OllamaTranslator::new(s.uri(), "llama3".into()).health_check().await.is_err());
}

#[tokio::test]
async fn ollama_unreachable_is_actionable_error() {
    let err = OllamaTranslator::new("http://127.0.0.1:9".into(), "x".into()).health_check().await.unwrap_err();
    assert!(format!("{err:#}").contains("ollama serve"));
}

#[tokio::test]
async fn openai_compat_sends_bearer_and_surfaces_http_errors() {
    let s = MockServer::start().await;
    Mock::given(path("/chat/completions")).and(header("authorization", "Bearer k"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices":[{"message":{"content":"[1] A\n[2] B"}}]})))
        .mount(&s).await;
    let t = OpenAiCompatTranslator::new(s.uri(), "k".into(), "m".into());
    assert_eq!(t.translate_batch(&batch(), "en", "vi").await.unwrap().len(), 2);
    let bad = OpenAiCompatTranslator::new(s.uri(), "wrong".into(), "m".into());
    assert!(bad.translate_batch(&batch(), "en", "vi").await.is_err()); // wiremock returns 404
}
```

- [ ] **Step 2: Run** `cargo test --test translator_test` → FAIL
- [ ] **Step 3: Implement.** Prompt template:

```text
You are a professional subtitle translator.
Translate each numbered subtitle line from {SRC} into {TARGET}.
Keep it natural and consistent with the dialogue context (pronouns, tone).
Return ONLY the translated lines, one per line, with the SAME numbers, e.g.:
[1] ...
[2] ...
Do not merge, split, skip or add lines. Keep " / " where it appears.

{NUMBERED_LINES}
```
- [ ] **Step 4: Run** tests + clippy → PASS
- [ ] **Step 5: Commit** `git commit -am "feat: batch translators for ollama and openai-compatible apis"`

---

### Task 7: Speech-to-Text (local whisper-rs + cloud API)

**Files:**
- Modify: `Cargo.toml` (add deps + `cuda` feature)
- Create: `src/stt/mod.rs`, `src/stt/audio.rs`, `src/stt/whisper_local.rs`, `src/stt/whisper_api.rs`; Modify: `src/lib.rs`
- Test: `tests/stt_test.rs`

**Interfaces:**
```rust
// mod.rs
#[async_trait::async_trait]
pub trait Transcriber: Send + Sync {
    /// Returns sanitized segments (ids 1..=n). Err on engine failure; Ok(vec![]) means no speech.
    async fn transcribe(&self, audio_path: &Path) -> anyhow::Result<Vec<SubtitleSegment>>;
}
// audio.rs
/// Err unless 16000 Hz, 1 channel, 16-bit int PCM. Samples scaled to [-1,1] by /32768.0.
pub fn read_wav_16k_mono(path: &Path) -> anyhow::Result<Vec<f32>>;
// whisper_local.rs
pub struct LocalWhisperTranscriber { model_path: PathBuf, language: Option<String>, use_gpu: bool }
impl LocalWhisperTranscriber { pub fn new(model_path: PathBuf, language: Option<String>, use_gpu: bool) -> Self; }
/// (t0_centiseconds, t1_centiseconds, text) -> segments in ms, then sanitize_segments.
pub fn segments_from_whisper(raw: Vec<(i64, i64, String)>) -> Vec<SubtitleSegment>;
// whisper_api.rs
pub struct ApiTranscriber { /* api_key, base_url, model, language, client */ }
impl ApiTranscriber { pub fn new(api_key: String, base_url: String, model: String, language: Option<String>) -> Self; }
pub const API_MAX_UPLOAD_BYTES: u64 = 25 * 1024 * 1024;
```

- [ ] **Step 1: Add dependencies**

```toml
[features]
default = []
cuda = ["whisper-rs/cuda"]

[dependencies]
# ...existing...
whisper-rs = "0.14"
hound = "3.5"
```
Run: `cargo build` → Expected: compiles whisper.cpp (needs Task 0 prerequisites; first build takes several minutes).

- [ ] **Step 2: Write failing tests**

```rust
use auto_subtitle::stt::{audio::read_wav_16k_mono, whisper_local::segments_from_whisper, whisper_api::ApiTranscriber, Transcriber};
use wiremock::{MockServer, Mock, ResponseTemplate, matchers::path};
use serde_json::json;

fn write_wav(p: &std::path::Path, rate: u32) {
    let spec = hound::WavSpec { channels: 1, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(p, spec).unwrap();
    for _ in 0..1600 { w.write_sample(16384i16).unwrap(); }
    w.finalize().unwrap();
}

#[test]
fn reads_valid_wav_and_rejects_wrong_rate() {
    let t = tempfile::tempdir().unwrap();
    let ok = t.path().join("ok.wav"); write_wav(&ok, 16000);
    let s = read_wav_16k_mono(&ok).unwrap();
    assert_eq!(s.len(), 1600);
    assert!((s[0] - 0.5).abs() < 1e-4);
    let bad = t.path().join("bad.wav"); write_wav(&bad, 44100);
    assert!(read_wav_16k_mono(&bad).is_err());
}

#[test]
fn whisper_centiseconds_to_ms() {
    let segs = segments_from_whisper(vec![(0, 150, " Hello ".into()), (150, 150, "empty-dur".into()), (200, 420, "World".into())]);
    assert_eq!(segs.len(), 2);
    assert_eq!((segs[0].start_ms, segs[0].end_ms, segs[0].original_text.as_str()), (0, 1500, "Hello"));
    assert_eq!((segs[1].id, segs[1].start_ms, segs[1].end_ms), (2, 2000, 4200));
}

#[tokio::test]
async fn api_transcriber_parses_verbose_json() {
    let s = MockServer::start().await;
    Mock::given(path("/audio/transcriptions")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "segments": [{"start": 0.0, "end": 1.25, "text": " Hi "}, {"start": 1.25, "end": 2.5, "text": "there"}]
    }))).mount(&s).await;
    let t = tempfile::tempdir().unwrap();
    let audio = t.path().join("a.mp3"); std::fs::write(&audio, b"fake").unwrap();
    let tr = ApiTranscriber::new("k".into(), s.uri(), "whisper-1".into(), None);
    let segs = tr.transcribe(&audio).await.unwrap();
    assert_eq!((segs[0].end_ms, segs[0].original_text.as_str()), (1250, "Hi"));
    assert_eq!(segs.len(), 2);
}

/// Requires a real model: AUTO_SUBTITLE_TEST_MODEL=/path/ggml-tiny.bin cargo test -- --ignored
#[tokio::test]
#[ignore]
async fn local_whisper_runs_on_silence() {
    use auto_subtitle::stt::whisper_local::LocalWhisperTranscriber;
    let model = std::env::var("AUTO_SUBTITLE_TEST_MODEL").expect("set AUTO_SUBTITLE_TEST_MODEL");
    let t = tempfile::tempdir().unwrap();
    let wav = t.path().join("s.wav"); write_wav(&wav, 16000);
    let tr = LocalWhisperTranscriber::new(model.into(), None, false);
    tr.transcribe(&wav).await.unwrap(); // must not error; may return empty
}
```

- [ ] **Step 3: Run** `cargo test --test stt_test` → FAIL
- [ ] **Step 4: Implement local engine** (verify method names against docs.rs for the resolved `whisper-rs 0.14.x`; the contract above is fixed, the calls below are the expected API):

```rust
#[async_trait]
impl Transcriber for LocalWhisperTranscriber {
    async fn transcribe(&self, audio_path: &Path) -> Result<Vec<SubtitleSegment>> {
        let samples = read_wav_16k_mono(audio_path)?;
        let (model, lang, gpu) = (self.model_path.clone(), self.language.clone(), self.use_gpu);
        tokio::task::spawn_blocking(move || {
            match run_whisper(&model, lang.as_deref(), &samples, gpu) {
                Ok(raw) => Ok(raw),
                Err(e) if gpu => {
                    eprintln!("⚠️ GPU inference failed ({e:#}); retrying on CPU...");
                    run_whisper(&model, lang.as_deref(), &samples, false)
                }
                Err(e) => Err(e),
            }
        }).await?.map(segments_from_whisper)
    }
}

fn run_whisper(model: &Path, lang: Option<&str>, samples: &[f32], use_gpu: bool) -> Result<Vec<(i64, i64, String)>> {
    use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
    let mut cp = WhisperContextParameters::default();
    cp.use_gpu(use_gpu);
    let model_str = model.to_str().context("model path is not valid UTF-8")?;
    let ctx = WhisperContext::new_with_params(model_str, cp).context("load whisper model")?;
    let mut state = ctx.create_state().context("create whisper state")?;
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    p.set_language(Some(lang.unwrap_or("auto")));
    p.set_n_threads(std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4));
    p.set_print_progress(false); p.set_print_realtime(false);
    p.set_print_special(false); p.set_print_timestamps(false);
    state.full(p, samples).context("whisper inference")?;
    let n = state.full_n_segments()?;
    (0..n).map(|i| Ok((state.full_get_segment_t0(i)?, state.full_get_segment_t1(i)?, state.full_get_segment_text(i)?)))
        .collect()
}
```

- [ ] **Step 5: Implement API engine:** check file size ≤ `API_MAX_UPLOAD_BYTES` (else `Err` suggesting `--stt-engine local`); multipart fields `file` (mime `audio/mpeg` for `.mp3`, `audio/wav` for `.wav`), `model`, `response_format=verbose_json`, optional `language`; POST `{base}/audio/transcriptions` with Bearer; `error_for_status`; map `segments[].start/end` (seconds, f64) × 1000 → ms; pass through `sanitize_segments`.
- [ ] **Step 6: Run** `cargo test --test stt_test` + clippy → PASS. If a model is available: `AUTO_SUBTITLE_TEST_MODEL=~/.cache/auto-subtitle/models/ggml-tiny.bin cargo test --test stt_test -- --ignored` → PASS.
- [ ] **Step 7: Verify CUDA build compiles (if nvcc installed):** `cargo build --release --features cuda` → success.
- [ ] **Step 8: Commit** `git commit -am "feat: local whisper-rs and cloud api transcribers with gpu fallback"`

---

### Task 8: CLI, Pipeline Orchestration & End-to-End Tests

**Files:**
- Create: `src/cli.rs`, `src/pipeline.rs`, `src/main.rs`; Modify: `src/lib.rs`
- Test: `tests/cli_test.rs`, `tests/pipeline_test.rs`

**Interfaces:**
```rust
// cli.rs  (in lib so tests can call Cli::try_parse_from)
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)] pub enum OutputMode { Hardsub, Softsub, SubsOnly }
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)] pub enum SttEngine { Local, Api }
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)] pub enum TranslateEngine { Ollama, OpenaiCompat }
#[derive(clap::Parser, Debug, Clone)]
pub struct Cli {
    #[arg(short, long)] pub input: PathBuf,
    #[arg(short, long)] pub output: Option<PathBuf>,
    #[arg(long = "subtitle", visible_alias = "sub")] pub subtitle: Option<PathBuf>,
    #[arg(long)] pub skip_translate: bool,
    #[arg(short = 's', long, default_value = "auto")] pub source_lang: String,
    #[arg(short = 't', long, default_value = "vi")] pub target_lang: String,
    #[arg(short, long, value_enum, default_value_t = OutputMode::Hardsub)] pub mode: OutputMode,
    #[arg(short, long)] pub bilingual: bool,
    #[arg(long, value_enum, default_value_t = SttEngine::Local)] pub stt_engine: SttEngine,
    #[arg(long, default_value = "small")] pub whisper_model: String,
    #[arg(long, default_value = "whisper-1")] pub stt_api_model: String,
    #[arg(long, value_enum, default_value_t = TranslateEngine::Ollama)] pub translate_engine: TranslateEngine,
    #[arg(long, default_value = "qwen2.5:7b")] pub ollama_model: String,
    #[arg(long, default_value = "http://localhost:11434")] pub ollama_url: String,
    #[arg(long, env = "OPENAI_API_KEY", hide_env_values = true)] pub api_key: Option<String>,
    #[arg(long, default_value = "https://api.openai.com/v1")] pub api_base_url: String,
    #[arg(long, default_value = "gpt-4o-mini")] pub api_model: String,
    #[arg(long)] pub keep_temp: bool,
}

// pipeline.rs
pub const BATCH_SIZE: usize = 20;
pub const MAX_ATTEMPTS: usize = 3;
pub struct OutputPaths { pub video: Option<PathBuf>, pub srt: PathBuf, pub ass: PathBuf }
/// Defaults: video = <input_dir>/<stem>_subtitled.<ext> (ext = input ext for softsub, "mp4" for hardsub; None for subs-only).
/// out_stem = stem of `-o` if given, else "<input_stem>_subtitled"; output_dir = parent of `-o` if given, else input dir.
/// srt/ass  = <output_dir>/<out_stem>.<target_lang>.srt|.ass  (exported for subs-only, or with --keep-temp).
/// Err if any output path canonically equals cli.input or cli.subtitle.
pub fn resolve_output_paths(cli: &Cli) -> anyhow::Result<OutputPaths>;
/// Batches of BATCH_SIZE; each batch retried up to MAX_ATTEMPTS; after that Err("translation failed for lines a–b: ...").
pub async fn translate_all(t: &dyn Translator, segs: &mut [SubtitleSegment], src: &str, tgt: &str) -> anyhow::Result<()>;
pub struct Deps<'a> {
    pub ffmpeg: Option<PathBuf>,               // None allowed only for subs-only + --subtitle
    pub transcriber: Option<&'a dyn Transcriber>, // None when --subtitle given
    pub translator: Option<&'a dyn Translator>,   // None when --skip-translate
}
pub struct Report { pub segments: usize, pub outputs: Vec<PathBuf> }
pub async fn run(cli: &Cli, deps: Deps<'_>) -> anyhow::Result<Report>;
```

Pipeline steps in `run` (each with an `indicatif` spinner/bar, messages as in spec §3 Phần 3):
1. Validate `cli.input` exists; `resolve_output_paths`; create `TempDir` work dir (`keep()` it if `--keep-temp`, print its path).
2. Segments: `--subtitle` → `parse_subtitle_file`; else extract audio (WAV for local, MP3 for API) into work dir via `media`, then `transcriber.transcribe`. Empty → `Err("No speech detected in <input>")`.
3. Unless `--skip-translate`: `translate_all`.
4. Write `work/subtitle.srt` and `work/subtitle.ass` (title = input stem).
5. Mode: `Hardsub` → `build_hardsub_command(ffmpeg, input, work/subtitle.ass, video)`; `Softsub` → `build_softsub_command(..., work/subtitle.srt, video)`; `SubsOnly` → copy both to `OutputPaths.srt/.ass`. With `--keep-temp` in video modes, also copy subs to `OutputPaths.srt/.ass`.
6. Return `Report`.

`main.rs` responsibilities only: parse `Cli`; `AppConfig::load()`; `DependencyManager`; ffmpeg via `find_or_download_ffmpeg` unless (subs-only && `--subtitle`); build transcriber (local: `ensure_whisper_model` + `LocalWhisperTranscriber::new(path, lang_opt, true)`; api: requires `api_key`); build translator (ollama: `health_check()` first; openai-compat: requires `api_key`, else `Err("--api-key or OPENAI_API_KEY required")`); call `pipeline::run`; print outputs; exit code 1 with `{err:#}` on failure.

- [ ] **Step 1: Write failing CLI tests** (`tests/cli_test.rs`)

```rust
use auto_subtitle::cli::*;
use clap::Parser;

#[test]
fn defaults_match_spec() {
    let c = Cli::try_parse_from(["auto-subtitle", "-i", "v.mp4"]).unwrap();
    assert_eq!(c.target_lang, "vi");
    assert_eq!(c.source_lang, "auto");
    assert_eq!(c.mode, OutputMode::Hardsub);
    assert_eq!(c.stt_engine, SttEngine::Local);
    assert_eq!(c.whisper_model, "small");
    assert_eq!(c.translate_engine, TranslateEngine::Ollama);
    assert_eq!(c.ollama_model, "qwen2.5:7b");
    assert!(!c.bilingual && !c.skip_translate && !c.keep_temp);
}

#[test]
fn sub_alias_and_modes() {
    let c = Cli::try_parse_from(["auto-subtitle", "-i", "v.mp4", "--sub", "s.srt", "--mode", "subs-only", "--skip-translate", "-b"]).unwrap();
    assert_eq!(c.subtitle.as_deref(), Some(std::path::Path::new("s.srt")));
    assert_eq!(c.mode, OutputMode::SubsOnly);
    assert!(c.skip_translate && c.bilingual);
}

#[test]
fn invalid_values_rejected() {
    assert!(Cli::try_parse_from(["auto-subtitle", "-i", "v.mp4", "--mode", "burn"]).is_err());
    assert!(Cli::try_parse_from(["auto-subtitle"]).is_err());
}
```

- [ ] **Step 2: Write failing pipeline tests** (`tests/pipeline_test.rs`)

```rust
use auto_subtitle::{cli::Cli, pipeline::*, stt::Transcriber, translate::Translator, subtitle::model::SubtitleSegment as S};
use async_trait::async_trait;
use clap::Parser;
use std::sync::atomic::{AtomicUsize, Ordering};

struct MockStt;
#[async_trait] impl Transcriber for MockStt {
    async fn transcribe(&self, _: &std::path::Path) -> anyhow::Result<Vec<S>> {
        Ok(vec![S::new(1, 0, 1000, "Hello"), S::new(2, 1000, 2500, "World")])
    }
}
struct Prefix;
#[async_trait] impl Translator for Prefix {
    async fn translate_batch(&self, s: &[S], _: &str, _: &str) -> anyhow::Result<Vec<String>> {
        Ok(s.iter().map(|x| format!("VI:{}", x.original_text)).collect())
    }
}
struct Flaky(AtomicUsize);
#[async_trait] impl Translator for Flaky {
    async fn translate_batch(&self, s: &[S], a: &str, b: &str) -> anyhow::Result<Vec<String>> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 { anyhow::bail!("transient") }
        Prefix.translate_batch(s, a, b).await
    }
}
struct AlwaysFail;
#[async_trait] impl Translator for AlwaysFail {
    async fn translate_batch(&self, _: &[S], _: &str, _: &str) -> anyhow::Result<Vec<String>> { anyhow::bail!("down") }
}

fn many(n: usize) -> Vec<S> { (0..n).map(|i| S::new(i + 1, i as u64 * 1000, i as u64 * 1000 + 900, format!("l{i}"))).collect() }

#[tokio::test]
async fn translate_all_covers_every_batch() {
    let mut s = many(45); // 3 batches: catches the old global-id bug
    translate_all(&Prefix, &mut s, "en", "vi").await.unwrap();
    assert!(s.iter().all(|x| x.translated_text.as_deref() == Some(&format!("VI:{}", x.original_text)[..])));
}

#[tokio::test]
async fn translate_all_retries_then_fails_loudly() {
    let mut s = many(3);
    translate_all(&Flaky(AtomicUsize::new(0)), &mut s, "en", "vi").await.unwrap();
    let err = translate_all(&AlwaysFail, &mut many(3), "en", "vi").await.unwrap_err();
    assert!(format!("{err:#}").contains("lines 1–3"));
}

#[tokio::test]
async fn subs_only_with_existing_subtitle_never_touches_input() {
    let t = tempfile::tempdir().unwrap();
    let video = t.path().join("clip.mp4"); std::fs::write(&video, b"not a real video").unwrap();
    let sub = t.path().join("clip.srt");
    let original = "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n";
    std::fs::write(&sub, original).unwrap();
    let cli = Cli::try_parse_from(["a", "-i", video.to_str().unwrap(), "--sub", sub.to_str().unwrap(), "--mode", "subs-only", "-b"]).unwrap();
    let r = run(&cli, Deps { ffmpeg: None, transcriber: None, translator: Some(&Prefix) }).await.unwrap();
    assert_eq!(std::fs::read_to_string(&sub).unwrap(), original);            // input untouched
    let out_srt = t.path().join("clip_subtitled.vi.srt");
    assert!(r.outputs.contains(&out_srt));
    assert!(std::fs::read_to_string(out_srt).unwrap().contains("Hello\nVI:Hello"));
}

#[test]
fn output_equal_to_input_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    let video = t.path().join("v.mp4"); std::fs::write(&video, b"x").unwrap();
    let cli = Cli::try_parse_from(["a", "-i", video.to_str().unwrap(), "-o", video.to_str().unwrap()]).unwrap();
    assert!(resolve_output_paths(&cli).is_err());
}

/// Real FFmpeg E2E with mock AI. Skips (passes) when ffmpeg is not installed.
#[tokio::test]
async fn hardsub_and_softsub_e2e_with_synthetic_video() {
    let Ok(ffmpeg) = which::which("ffmpeg") else { eprintln!("skip: ffmpeg not found"); return; };
    let t = tempfile::tempdir().unwrap();
    let video = t.path().join("syn.mp4");
    let st = std::process::Command::new(&ffmpeg).args(["-hide_banner","-loglevel","error","-y",
        "-f","lavfi","-i","testsrc=duration=3:size=320x240:rate=25",
        "-f","lavfi","-i","sine=frequency=440:duration=3",
        "-c:v","libx264","-c:a","aac","-shortest"]).arg(&video).status().unwrap();
    assert!(st.success());
    for mode in ["hardsub", "softsub"] {
        let out = t.path().join(format!("out_{mode}.mp4"));
        let cli = Cli::try_parse_from(["a","-i",video.to_str().unwrap(),"-o",out.to_str().unwrap(),"--mode",mode]).unwrap();
        run(&cli, Deps { ffmpeg: Some(ffmpeg.clone()), transcriber: Some(&MockStt), translator: Some(&Prefix) }).await.unwrap();
        assert!(std::fs::metadata(&out).unwrap().len() > 0, "{mode} output missing");
    }
}
```

- [ ] **Step 3: Run** `cargo test --test cli_test --test pipeline_test` → FAIL
- [ ] **Step 4: Implement** `cli.rs`, `pipeline.rs`, `main.rs`; `lib.rs` exports `cli, config, downloader, media, pipeline, stt, subtitle, translate`.
- [ ] **Step 5: Run full gate**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
Expected: all pass (E2E prints `skip` if ffmpeg absent — after the first real run, ffmpeg exists in cache but not PATH; to exercise it run `PATH=~/.cache/auto-subtitle/bin:$PATH cargo test`).

- [ ] **Step 6: Manual smoke test (real models)**

```bash
cargo run --release -- -i sample.mp4 --mode subs-only                 # downloads ffmpeg + ggml-small, uses Ollama
cargo run --release -- -i sample.mp4 --bilingual                      # hardsub
cargo run --release -- -i sample.mp4 --sub sample.srt --skip-translate --mode softsub
cargo run --release --features cuda -- -i sample.mp4                  # GPU (if nvcc installed)
```
Expected: `sample_subtitled.mp4` plays with correct Vietnamese subtitles; `sample.srt` unchanged.

- [ ] **Step 7: Commit** `git commit -am "feat: cli and pipeline orchestration with e2e tests"`

---

## Spec Traceability

| Spec requirement | Task |
|---|---|
| §1 Hybrid local/cloud STT | 7, 8 |
| §1 Existing subtitle input + `--skip-translate` | 2, 8 |
| §3 `SubtitleSegment`, `Transcriber`, `Translator` | 1, 6, 7 |
| §4 CLI flags (+ added `--stt-api-model`, `--api-model`) | 8 |
| §5 Zero-config FFmpeg / model download | 4 |
| §6.1 Ollama health check | 6 |
| §6.2 CUDA → CPU fallback | 7 |
| §6.3 Timecode sanitization, ASS tag escaping | 1, 3 |
| §7 Unit tests + E2E with mocks | all; E2E in 8 |
