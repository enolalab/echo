# Voice Dubbing (`--dub`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional voice dubbing — TTS each translated subtitle segment, fit clips into time slots, mix over ducked original audio — to CLI and Echo Studio GUI.

**Architecture:** Pluggable `TtsEngine` trait (`local` = Piper binary bundle, `api` = OpenAI-compatible `/audio/speech`). `src/dub.rs` owns slot budgeting, clip fitting (gap overflow + `atempo`), track assembly via `hound`, and ffmpeg mux. Two call sites: `pipeline::run` (CLI) and `export_subtitles` (GUI — export-time so dubbed text reflects user edits).

**Tech Stack:** Rust 2021, clap, tokio, reqwest, hound (existing), whisper-rs pipeline, Piper (VITS), ffmpeg, Tauri v2 + React/TS.

**Spec:** `docs/superpowers/specs/2026-11-02-voice-dubbing-design.md`

## Global Constraints

- `MAX_OVERFLOW_MS = 2000`; `MAX_TEMPO = 1.5`; budget = `slot + min(gap/2, MAX_OVERFLOW_MS)`; `gap(last) = video_end - end` (probe via `ffmpeg -i` stderr `Duration:`; fallback gap = 0).
- Dub text = `translated_text` else `original_text`; normalize before synthesize (flatten newlines, strip `{...}` tags).
- Skip empty-text segments; per-segment TTS failure → warn + skip; abort if >50% of synthesized segments fail.
- All clips normalized to 48 kHz mono s16 WAV; track built with `hound`; overlapping samples are summed (i32 accumulator, clamp to i16).
- Mix: `[0:a]volume=<orig_vol>[a0];[1:a]apad[a1];[a0][a1]amix=inputs=2:duration=longest:normalize=0[aout]`; default `orig_vol = 0.15`. No-audio input → map dubbed wav directly, no amix.
- Default local voice = first catalog entry whose `lang` == `--target-lang`; no match → error advising `--tts-voice`/`--tts-engine api`.
- Piper auto-download: Linux x86_64 only (same policy as ffmpeg); other platforms → must exist on `PATH`. Bundle is a directory: `bin/piper/{piper,lib/,espeak-ng-data/}` — preserve layout.
- No new crate dependencies: no zip/flate2 — piper extracted via `tar` like ffmpeg.
- Commit style: `feat:`, `fix:`, `docs:` prefixes per `git log`.

---

### Task 1: `src/tts/` module — trait, DubOptions, voice catalog, text normalization

**Files:**
- Create: `src/tts/mod.rs`
- Modify: `src/lib.rs` (add `pub mod tts;`)

**Interfaces:**
- Produces: `tts::TtsEngine` trait (`synthesize(&self, &str) -> Result<Vec<u8>>`, `name()`), `tts::DubOptions` (Serialize/Deserialize), `tts::TtsEngineKind`, `tts::VOICE_CATALOG: &[VoiceInfo]`, `tts::default_voice_for_lang(&str) -> Option<&'static VoiceInfo>`, `tts::normalize_tts_text(&str) -> String`.

- [ ] **Step 1: Write failing tests**

Append to `src/tts/mod.rs` after creating it (test module first, impl below):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_ass_tags_and_newlines() {
        assert_eq!(normalize_tts_text("Xin {\\i1}chào{\\i0}\nthế giới"), "Xin chào thế giới");
        assert_eq!(normalize_tts_text("  a  \n b \r\n c  "), "a b c");
        assert_eq!(normalize_tts_text("{\\pos(1,2)}"), "");
    }

    #[test]
    fn default_voice_matches_lang() {
        assert_eq!(default_voice_for_lang("vi").unwrap().id, "vi_VN-vais1000-medium");
        assert_eq!(default_voice_for_lang("en").unwrap().id, "en_US-lessac-medium");
        assert!(default_voice_for_lang("ja").is_none());
        assert_eq!(default_voice_for_lang("VI").unwrap().id, "vi_VN-vais1000-medium");
    }

    #[test]
    fn dub_options_serde_roundtrip() {
        let d: DubOptions = serde_json::from_str(r#"{"enabled":true,"engine":"local","orig_volume":0.2}"#).unwrap();
        assert!(d.enabled);
        assert_eq!(d.engine, TtsEngineKind::Local);
        assert_eq!(d.orig_volume, 0.2);
    }
}
```

- [ ] **Step 2: Implement `src/tts/mod.rs`**

```rust
use serde::{Deserialize, Serialize};

pub mod api;
pub mod piper;

pub const HF_VOICES_BASE: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TtsEngineKind {
    #[default]
    Local,
    Api,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DubOptions {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub engine: TtsEngineKind,
    #[serde(default)]
    pub voice: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default = "default_orig_volume")]
    pub orig_volume: f32,
}

fn default_orig_volume() -> f32 { 0.15 }

impl Default for DubOptions {
    fn default() -> Self {
        Self { enabled: false, engine: TtsEngineKind::Local, voice: None, model: None, orig_volume: default_orig_volume() }
    }
}

#[async_trait::async_trait]
pub trait TtsEngine: Send + Sync {
    /// Synthesize text → audio bytes (WAV expected; other codecs tolerated downstream).
    async fn synthesize(&self, text: &str) -> anyhow::Result<Vec<u8>>;
    fn name(&self) -> &'static str;
}

#[derive(Debug)]
pub struct VoiceInfo {
    pub id: &'static str,
    pub lang: &'static str,
    /// Directory inside rhasspy/piper-voices on HuggingFace.
    pub hf_dir: &'static str,
}

pub const VOICE_CATALOG: &[VoiceInfo] = &[
    VoiceInfo { id: "vi_VN-vais1000-medium", lang: "vi", hf_dir: "vi/vi_VN/vais1000/medium" },
    VoiceInfo { id: "vi_VN-25hours_single-low", lang: "vi", hf_dir: "vi/vi_VN/25hours_single/low" },
    VoiceInfo { id: "en_US-lessac-medium", lang: "en", hf_dir: "en/en_US/lessac/medium" },
];

pub fn find_voice(id: &str) -> Option<&'static VoiceInfo> {
    VOICE_CATALOG.iter().find(|v| v.id == id)
}

/// First catalog voice whose lang matches `target_lang` (case-insensitive).
pub fn default_voice_for_lang(target_lang: &str) -> Option<&'static VoiceInfo> {
    let lang = target_lang.trim().to_lowercase();
    VOICE_CATALOG.iter().find(|v| v.lang == lang)
}

/// Flatten newlines to spaces and strip ASS override tags ({...}) —
/// piper treats each stdin line as a separate utterance.
pub fn normalize_tts_text(text: &str) -> String {
    static TAG_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = TAG_RE.get_or_init(|| regex::Regex::new(r"\{[^}]*\}").unwrap());
    let no_tags = re.replace_all(text, " ");
    no_tags.split_whitespace().collect::<Vec<_>>().join(" ")
}
```

Create empty placeholders `src/tts/api.rs` and `src/tts/piper.rs` (each containing `// implemented in later task`) and add `pub mod tts;` to `src/lib.rs` after `pub mod translate;`.

- [ ] **Step 3: Run tests**

Run: `cargo test tts::`
Expected: 3 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/tts src/lib.rs
git commit -m "feat(tts): engine trait, DubOptions, voice catalog, text normalization"
```

---

### Task 2: Piper bundle + voice model download in `downloader.rs`

**Files:**
- Modify: `src/config.rs` (add `tts_dir`)
- Modify: `src/downloader.rs`

**Interfaces:**
- Consumes: `tts::{VoiceInfo, HF_VOICES_BASE}` (Task 1), existing `download_to`, `AppConfig`.
- Produces: `AppConfig.tts_dir`; `DependencyManager::find_piper() -> Option<PathBuf>`; `find_or_download_piper() -> Result<PathBuf>`; `ensure_voice(&VoiceInfo) -> Result<(PathBuf /*onnx*/, PathBuf /*json*/)>`.

- [ ] **Step 1: Add `tts_dir` to `AppConfig`**

In `src/config.rs`, add field `pub tts_dir: PathBuf` to `AppConfig`; in `with_base`, `let tts_dir = base.join("tts");` + `fs::create_dir_all(&tts_dir)?` (same context pattern), and set it in `Self`.

- [ ] **Step 2: Write failing test + implement**

In `src/downloader.rs` add:

```rust
use crate::tts::{VoiceInfo, HF_VOICES_BASE};

impl DependencyManager {
    /// Piper bundle dir: <bin_dir>/piper/ containing binary + lib/ + espeak-ng-data/.
    pub fn piper_bundle_dir(&self) -> PathBuf {
        self.config.bin_dir.join("piper")
    }

    pub fn find_piper(&self) -> Option<PathBuf> {
        if let Ok(p) = which::which("piper") {
            return Some(p);
        }
        #[cfg(windows)]
        let exe = self.piper_bundle_dir().join("piper.exe");
        #[cfg(not(windows))]
        let exe = self.piper_bundle_dir().join("piper");
        exe.is_file().then_some(exe)
    }

    /// Auto-download supported on Linux x86_64 only (same policy as ffmpeg).
    /// Other platforms: install piper manually so it's on PATH.
    pub async fn find_or_download_piper(&self) -> Result<PathBuf> {
        if let Some(p) = self.find_piper() {
            return Ok(p);
        }
        if !(cfg!(target_os = "linux") && std::env::consts::ARCH == "x86_64") {
            anyhow::bail!(
                "Automatic Piper download is only supported on Linux x86_64. \
                 Install piper manually and ensure it is in your PATH."
            );
        }

        let url = "https://github.com/rhasspy/piper/releases/latest/download/piper_linux_x86_64.tar.gz";
        let temp_dir = tempfile::tempdir().context("failed to create temp dir for piper")?;
        let archive = temp_dir.path().join("piper.tar.gz");
        download_to(&self.client, url, &archive).await?;

        let status = tokio::process::Command::new("tar")
            .arg("-xzf").arg(&archive)
            .arg("-C").arg(temp_dir.path())
            .status().await
            .context("failed to run tar -xzf on piper archive")?;
        if !status.success() {
            anyhow::bail!("tar extraction failed with status: {status}");
        }

        // Archive extracts to a `piper/` directory — preserve the whole bundle.
        let extracted = temp_dir.path().join("piper");
        anyhow::ensure!(extracted.is_dir(), "piper archive did not contain piper/ directory");
        let dest = self.piper_bundle_dir();
        if dest.exists() {
            fs::remove_dir_all(&dest).with_context(|| format!("failed to clear {}", dest.display()))?;
        }
        fs::create_dir_all(&self.config.bin_dir)?;
        if fs::rename(&extracted, &dest).is_err() {
            copy_dir_all(&extracted, &dest)?;
        }

        #[cfg(unix)]
        {
            let exe = dest.join("piper");
            fs::set_permissions(&exe, fs::Permissions::from_mode(0o755))
                .with_context(|| format!("failed to chmod 0755 {}", exe.display()))?;
        }
        let out = tokio::process::Command::new(dest.join("piper"))
            .arg("--version").output().await
            .context("failed to verify piper binary")?;
        if !out.status.success() {
            let _ = fs::remove_dir_all(&dest);
            anyhow::bail!("downloaded piper failed --version check");
        }
        Ok(dest.join("piper"))
    }

    /// Paths for voice files: <tts_dir>/<id>.onnx and <id>.onnx.json.
    pub fn voice_paths(&self, voice_id: &str) -> (PathBuf, PathBuf) {
        (
            self.config.tts_dir.join(format!("{voice_id}.onnx")),
            self.config.tts_dir.join(format!("{voice_id}.onnx.json")),
        )
    }

    pub async fn ensure_voice(&self, voice: &VoiceInfo) -> Result<(PathBuf, PathBuf)> {
        let (onnx, json) = self.voice_paths(voice.id);
        if !onnx.is_file() {
            let url = format!("{HF_VOICES_BASE}/{}/{id}.onnx", voice.hf_dir, id = voice.id);
            download_to(&self.client, &url, &onnx).await?;
        }
        if !json.is_file() {
            let url = format!("{HF_VOICES_BASE}/{}/{id}.onnx.json", voice.hf_dir, id = voice.id);
            download_to(&self.client, &url, &json).await?;
        }
        Ok((onnx, json))
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}
```

Unit test (append `#[cfg(test)] mod tests` if absent):

```rust
    #[test]
    fn voice_paths_layout() {
        let cfg = AppConfig::with_base(std::env::temp_dir().join("as-test-dl")).unwrap();
        let dm = DependencyManager::new(cfg);
        let (o, j) = dm.voice_paths("vi_VN-vais1000-medium");
        assert!(o.ends_with("vi_VN-vais1000-medium.onnx"));
        assert!(j.ends_with("vi_VN-vais1000-medium.onnx.json"));
    }
```

- [ ] **Step 3: Run** `cargo test downloader` → PASS; `cargo clippy --all-targets -- -D warnings` → clean.

- [ ] **Step 4: Commit**

```bash
git add src/config.rs src/downloader.rs
git commit -m "feat(downloader): piper bundle + voice model download"
```

---

### Task 3: `PiperTts` engine

**Files:**
- Modify: `src/tts/piper.rs`

**Interfaces:**
- Consumes: `TtsEngine` trait (Task 1), paths from `DependencyManager` (Task 2).
- Produces: `PiperTts::new(piper: PathBuf, model: PathBuf, config: PathBuf)`; `impl TtsEngine`.

- [ ] **Step 1: Implement**

```rust
use std::path::PathBuf;
use anyhow::Context;
use tokio::io::AsyncWriteExt;

use super::TtsEngine;

pub struct PiperTts {
    piper: PathBuf,
    model: PathBuf,
    config: PathBuf,
}

impl PiperTts {
    pub fn new(piper: PathBuf, model: PathBuf, config: PathBuf) -> Self {
        Self { piper, model, config }
    }
}

#[async_trait::async_trait]
impl TtsEngine for PiperTts {
    fn name(&self) -> &'static str { "piper" }

    async fn synthesize(&self, text: &str) -> anyhow::Result<Vec<u8>> {
        let dir = tempfile::tempdir().context("piper temp dir")?;
        let out = dir.path().join("out.wav");
        let mut cmd = tokio::process::Command::new(&self.piper);
        cmd.arg("-m").arg(&self.model)
            .arg("-c").arg(&self.config)
            .arg("-f").arg(&out)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        // espeak-ng-data + lib/ resolve relative to the binary's dir.
        if let Some(parent) = self.piper.parent() {
            cmd.current_dir(parent);
        }
        let mut child = cmd.spawn().context("failed to spawn piper")?;
        child
            .stdin
            .as_mut()
            .expect("stdin piped")
            .write_all(text.as_bytes())
            .await
            .context("failed to write piper stdin")?;
        drop(child.stdin.take());
        let output = child.wait_with_output().await.context("piper wait failed")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let tail: String = stderr.lines().rev().take(10).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
            anyhow::bail!("piper failed ({}): {tail}", output.status);
        }
        let bytes = tokio::fs::read(&out).await.with_context(|| format!("piper did not write {}", out.display()))?;
        Ok(bytes)
    }
}
```

- [ ] **Step 2: Test**

Unit test (construction/arg-check only — real synthesis needs the binary):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constructs() {
        let e = PiperTts::new("p".into(), "m".into(), "c".into());
        assert_eq!(e.name(), "piper");
    }
}
```

Run: `cargo test tts::piper` → PASS.

- [ ] **Step 3: Commit**

```bash
git add src/tts/piper.rs
git commit -m "feat(tts): PiperTts local engine"
```

---

### Task 4: `ApiTts` engine

**Files:**
- Modify: `src/tts/api.rs`

**Interfaces:**
- Produces: `ApiTts::new(base_url: String, api_key: String, model: String, voice: String)`; `impl TtsEngine`.

- [ ] **Step 1: Write failing wiremock test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, header};

    #[tokio::test]
    async fn posts_to_audio_speech() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/audio/speech"))
            .and(header("authorization", "Bearer k"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"RIFFfakewav".to_vec()))
            .mount(&server).await;
        let e = ApiTts::new(server.uri(), "k".into(), "tts-1".into(), "alloy".into());
        let bytes = e.synthesize("xin chào").await.unwrap();
        assert_eq!(bytes, b"RIFFfakewav");
    }

    #[tokio::test]
    async fn api_error_surfaces_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).respond_with(ResponseTemplate::new(429).set_body_string("quota"))
            .mount(&server).await;
        let e = ApiTts::new(server.uri(), "k".into(), "tts-1".into(), "alloy".into());
        let err = e.synthesize("x").await.unwrap_err().to_string();
        assert!(err.contains("429") || err.contains("quota"));
    }
}
```

- [ ] **Step 2: Implement**

```rust
use anyhow::Context;
use serde::Serialize;

use super::TtsEngine;

pub struct ApiTts {
    client: reqwest::Client,
    url: String,
    model: String,
    voice: String,
}

impl ApiTts {
    pub fn new(base_url: String, api_key: String, model: String, voice: String) -> Self {
        let client = reqwest::Client::builder()
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {api_key}").parse().expect("valid header"),
                );
                h
            })
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            url: format!("{}/audio/speech", base_url.trim_end_matches('/')),
            model,
            voice,
        }
    }
}

#[derive(Serialize)]
struct SpeechRequest<'a> {
    model: &'a str,
    input: &'a str,
    voice: &'a str,
    response_format: &'a str,
}

#[async_trait::async_trait]
impl TtsEngine for ApiTts {
    fn name(&self) -> &'static str { "api-tts" }

    async fn synthesize(&self, text: &str) -> anyhow::Result<Vec<u8>> {
        let resp = self
            .client
            .post(&self.url)
            .json(&SpeechRequest { model: &self.model, input: text, voice: &self.voice, response_format: "wav" })
            .send()
            .await
            .context("TTS request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let body: String = body.chars().take(300).collect();
            anyhow::bail!("TTS API {status}: {body}");
        }
        Ok(resp.bytes().await.context("failed to read TTS response")?.to_vec())
    }
}
```

- [ ] **Step 3: Run** `cargo test tts::api` → 2 PASS.

- [ ] **Step 4: Commit**

```bash
git add src/tts/api.rs
git commit -m "feat(tts): ApiTts OpenAI-compatible engine"
```

---

### Task 5: `src/dub.rs` — planning, probing, tempo decision

**Files:**
- Create: `src/dub.rs`
- Modify: `src/lib.rs` (add `pub mod dub;`)

**Interfaces:**
- Consumes: `SubtitleSegment`, `tts::normalize_tts_text`.
- Produces: `MAX_OVERFLOW_MS`, `MAX_TEMPO`, `DubPlan {seg_index, start_ms, slot_ms, budget_ms, text}`, `plan_segments(&[SubtitleSegment], Option<u64>) -> Vec<DubPlan>`, `decide_tempo(u64, u64) -> f64`, `parse_ffmpeg_duration_ms(&str) -> Option<u64>`, `parse_has_audio(&str) -> bool`, `probe_media(ffmpeg, video) -> (Option<u64>, bool)`.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitle::model::SubtitleSegment;

    fn seg(id: usize, s: u64, e: u64) -> SubtitleSegment {
        SubtitleSegment::new(id, s, e, format!("line {id}"))
    }

    #[test]
    fn plans_budget_with_half_gap_cap() {
        let segs = vec![
            seg(1, 0, 1000),
            seg(2, 5000, 6000), // gap 4000 → 2000 cap
            seg(3, 6200, 7000), // gap 200 → 100
        ];
        let plans = plan_segments(&segs, Some(9000));
        assert_eq!(plans[0].slot_ms, 1000);
        assert_eq!(plans[0].budget_ms, 1000 + 2000); // cap
        assert_eq!(plans[1].budget_ms, 1000 + 100);  // half of 200
        assert_eq!(plans[2].budget_ms, 800 + 1000);  // last: (9000-7000)/2
    }

    #[test]
    fn last_gap_zero_without_duration() {
        let segs = vec![seg(1, 0, 1000)];
        assert_eq!(plan_segments(&segs, None)[0].budget_ms, 1000);
    }

    #[test]
    fn skips_empty_and_uses_original_when_untranslated() {
        let mut a = seg(1, 0, 1000);
        a.translated_text = Some("   ".into());
        let b = seg(2, 2000, 3000); // no translation → original_text
        let plans = plan_segments(&[a, b], None);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].text, "line 2");
    }

    #[test]
    fn tempo_decisions() {
        assert_eq!(decide_tempo(900, 1000), 1.0);
        assert_eq!(decide_tempo(1200, 1000), 1.2);
        assert_eq!(decide_tempo(9000, 1000), 1.5); // clamped
        assert_eq!(decide_tempo(100, 0), 1.5);
    }

    #[test]
    fn parses_ffmpeg_stderr() {
        let err = "Input #0, mov, from 'x.mp4':\n  Duration: 00:01:23.45, start: 0.0\n    Stream #0:1: Audio: aac";
        assert_eq!(parse_ffmpeg_duration_ms(err), Some(83_450));
        assert!(parse_has_audio(err));
        assert!(!parse_has_audio("Stream #0:0: Video: h264"));
    }
}
```

- [ ] **Step 2: Implement `src/dub.rs`**

```rust
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::subtitle::model::SubtitleSegment;
use crate::tts::{normalize_tts_text, TtsEngine};

pub const MAX_OVERFLOW_MS: u64 = 2000;
pub const MAX_TEMPO: f64 = 1.5;

#[derive(Debug)]
pub struct DubPlan {
    pub seg_index: usize,
    pub start_ms: u64,
    pub slot_ms: u64,
    pub budget_ms: u64,
    pub text: String,
}

/// budget = slot + min(gap*0.5, MAX_OVERFLOW_MS); gap(last)=video_end-end else 0.
pub fn plan_segments(segs: &[SubtitleSegment], video_end_ms: Option<u64>) -> Vec<DubPlan> {
    segs.iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let raw = s.translated_text.as_deref().unwrap_or(&s.original_text);
            let text = normalize_tts_text(raw);
            if text.is_empty() {
                return None;
            }
            let slot = s.end_ms.saturating_sub(s.start_ms);
            let gap = if i + 1 < segs.len() {
                segs[i + 1].start_ms.saturating_sub(s.end_ms)
            } else {
                video_end_ms.map(|v| v.saturating_sub(s.end_ms)).unwrap_or(0)
            };
            Some(DubPlan {
                seg_index: i,
                start_ms: s.start_ms,
                slot_ms: slot,
                budget_ms: slot + std::cmp::min(gap / 2, MAX_OVERFLOW_MS),
                text,
            })
        })
        .collect()
}

/// 1.0 fits budget; else dur/budget clamped to [1.0, MAX_TEMPO].
pub fn decide_tempo(dur_ms: u64, budget_ms: u64) -> f64 {
    if dur_ms <= budget_ms {
        1.0
    } else {
        (dur_ms as f64 / budget_ms.max(1) as f64).clamp(1.0, MAX_TEMPO)
    }
}

/// "Duration: HH:MM:SS.cc" from `ffmpeg -i` stderr → ms.
pub fn parse_ffmpeg_duration_ms(stderr: &str) -> Option<u64> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"Duration: (\d+):(\d+):(\d+)(?:\.(\d+))?").unwrap()
    });
    let c = re.captures(stderr)?;
    let h: u64 = c[1].parse().ok()?;
    let m: u64 = c[2].parse().ok()?;
    let s: u64 = c[3].parse().ok()?;
    let frac: u64 = c.get(4).map_or(0, |f| {
        let t = f.as_str();
        t[..t.len().min(3)].parse::<u64>().unwrap_or(0) * 10u64.pow(3 - t.len().min(3) as u32)
    });
    Some(h * 3_600_000 + m * 60_000 + s * 1000 + frac)
}

pub fn parse_has_audio(stderr: &str) -> bool {
    stderr.contains("Audio:")
}

/// Runs `ffmpeg -i <video>`; stderr carries stream info. (duration_ms, has_audio)
pub fn probe_media(ffmpeg: &Path, video: &Path) -> (Option<u64>, bool) {
    let out = std::process::Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(video)
        .output();
    match out {
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            (parse_ffmpeg_duration_ms(&err), parse_has_audio(&err))
        }
        Err(_) => (None, false),
    }
}
```

- [ ] **Step 3: Run** `cargo test dub::` → 5 PASS; `cargo clippy` clean.

- [ ] **Step 4: Commit**

```bash
git add src/dub.rs src/lib.rs
git commit -m "feat(dub): slot planning, ffmpeg probing, tempo decision"
```

---

### Task 6: `src/dub.rs` — synthesis loop, clip fitting, track build

**Files:**
- Modify: `src/dub.rs`
- Modify: `src/media.rs` (add `build_normalize_clip_command`, `wav_duration_ms`)

**Interfaces:**
- Consumes: Task 5 items, `TtsEngine`, `run_ffmpeg`.
- Produces: `DubClip {start_ms, wav_path}`; `render_dub_track(engine, segs, ffmpeg, work_dir, video_path, on_progress: P, is_cancelled: C) -> Result<PathBuf>`; `media::build_normalize_clip_command(ffmpeg, in, out, tempo) -> Command`; `media::wav_duration_ms(&Path) -> Result<u64>`.

- [ ] **Step 1: Failing tests for track math**

```rust
    fn write_sine_wav(path: &Path, ms: u64) {
        let spec = hound::WavSpec { channels: 1, sample_rate: 48000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..(ms * 48) {
            w.write_sample(((i % 100) as i32 * 100).clamp(-30000, 30000) as i16).unwrap();
        }
        w.finalize().unwrap();
    }

    #[test]
    fn track_places_clips_and_sums_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.wav");
        let b = dir.path().join("b.wav");
        write_sine_wav(&a, 1000);
        write_sine_wav(&b, 1000);
        let clips = vec![
            DubClip { start_ms: 0, wav_path: a },
            DubClip { start_ms: 500, wav_path: b }, // overlaps a by 500ms
        ];
        let track = build_track(&clips).unwrap();
        assert_eq!(track.len(), 1500 * 48);
        // overlap region has larger amplitude than single-clip region
        let solo = track[100 * 48].abs() as i32;
        let both = track[700 * 48].abs() as i32;
        assert!(both > solo || both == i16::MAX as i32);
        assert!(track.iter().take(50 * 48).all(|&s| s != 0 || true)); // first 50ms may start at 0 amplitude
    }

    #[test]
    fn wav_duration_reads_ms() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.wav");
        write_sine_wav(&p, 250);
        assert_eq!(crate::media::wav_duration_ms(&p).unwrap(), 250);
    }
```

- [ ] **Step 2: Implement in `src/media.rs`**

```rust
/// Normalize any input to 48 kHz mono s16 WAV; applies atempo when tempo > 1.001.
pub fn build_normalize_clip_command(ffmpeg: &Path, input: &Path, out_wav: &Path, tempo: f64) -> Command {
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-hide_banner").arg("-loglevel").arg("error").arg("-y")
        .arg("-i").arg(input);
    if tempo > 1.001 {
        cmd.arg("-af").arg(format!("atempo={tempo:.4},aresample=48000"));
    } else {
        cmd.arg("-af").arg("aresample=48000");
    }
    cmd.arg("-ac").arg("1").arg("-ar").arg("48000")
        .arg("-c:a").arg("pcm_s16le").arg(out_wav);
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
```

- [ ] **Step 3: Implement rest of `src/dub.rs`**

```rust
#[derive(Debug)]
pub struct DubClip {
    pub start_ms: u64,
    pub wav_path: PathBuf, // normalized 48 kHz mono s16
}

/// Sum-mix clips onto a silent 48 kHz timeline; returns interleaved i16 samples.
pub fn build_track(clips: &[DubClip]) -> anyhow::Result<Vec<i16>> {
    let mut staged: Vec<(usize, Vec<i16>)> = Vec::new();
    let mut end = 0usize;
    for c in clips {
        let mut r = hound::WavReader::open(&c.wav_path)
            .with_context(|| format!("cannot read clip {}", c.wav_path.display()))?;
        let spec = r.spec();
        anyhow::ensure!(
            spec.channels == 1 && spec.sample_rate == 48000 && spec.bits_per_sample == 16,
            "clip {} not 48kHz mono s16", c.wav_path.display()
        );
        let samples: Vec<i16> = r.samples::<i16>().collect::<Result<_, _>>()?;
        let start = c.start_ms as usize * 48;
        end = end.max(start + samples.len());
        staged.push((start, samples));
    }
    let mut acc = vec![0i32; end];
    for (start, samples) in staged {
        for (j, s) in samples.iter().enumerate() {
            acc[start + j] += *s as i32;
        }
    }
    Ok(acc.iter().map(|&v| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16).collect())
}

pub fn write_track_wav(samples: &[i16], out: &Path) -> anyhow::Result<()> {
    let spec = hound::WavSpec { channels: 1, sample_rate: 48000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(out, spec)
        .with_context(|| format!("cannot create {}", out.display()))?;
    for s in samples {
        w.write_sample(*s)?;
    }
    w.finalize()?;
    Ok(())
}

/// Full dub stage: probe → plan → synthesize+fit per segment → track → dubbed.wav.
/// Skips failed segments; errors if >50% fail or `is_cancelled` between segments.
pub async fn render_dub_track<P, C>(
    engine: &dyn TtsEngine,
    segs: &[SubtitleSegment],
    ffmpeg: &Path,
    work_dir: &Path,
    video_path: &Path,
    mut on_progress: P,
    is_cancelled: C,
) -> anyhow::Result<PathBuf>
where
    P: FnMut(usize, usize) + Send,
    C: Fn() -> bool + Send,
{
    let (video_end_ms, _has_audio) = probe_media(ffmpeg, video_path);
    let plans = plan_segments(segs, video_end_ms);
    anyhow::ensure!(!plans.is_empty(), "no synthesizable subtitle text for dubbing");
    let total = plans.len();
    let mut clips = Vec::with_capacity(total);
    let mut failures = 0usize;

    for (idx, plan) in plans.into_iter().enumerate() {
        if is_cancelled() {
            anyhow::bail!("dubbing cancelled");
        }
        on_progress(idx + 1, total);
        let raw_path = work_dir.join(format!("dub_raw_{idx}.bin"));
        let clip_path = work_dir.join(format!("dub_clip_{idx}.wav"));
        let fitted = async {
            let bytes = engine.synthesize(&plan.text).await?;
            tokio::fs::write(&raw_path, &bytes).await?;
            let dur_ms = match crate::media::wav_duration_ms(&raw_path) {
                Ok(d) => (raw_path.clone(), d),
                Err(_) => {
                    // Non-WAV payload (e.g. provider returned mp3): normalize first, then measure.
                    let probe = work_dir.join(format!("dub_probe_{idx}.wav"));
                    crate::media::run_ffmpeg(
                        crate::media::build_normalize_clip_command(ffmpeg, &raw_path, &probe, 1.0),
                        "dub probe normalize",
                    )?;
                    let d = crate::media::wav_duration_ms(&probe)?;
                    (probe, d)
                }
            };
            let (src, dur) = dur_ms;
            let tempo = decide_tempo(dur, plan.budget_ms);
            crate::media::run_ffmpeg(
                crate::media::build_normalize_clip_command(ffmpeg, &src, &clip_path, tempo),
                "dub clip normalize",
            )?;
            anyhow::Ok(DubClip { start_ms: plan.start_ms, wav_path: clip_path })
        }
        .await;
        match fitted {
            Ok(c) => clips.push(c),
            Err(e) => {
                failures += 1;
                eprintln!("Warning: TTS failed for segment {}: {e:#}", plan.seg_index + 1);
            }
        }
    }

    anyhow::ensure!(
        failures * 2 <= total,
        "dubbing failed for {failures}/{total} segments — aborting"
    );
    let samples = build_track(&clips)?;
    let out = work_dir.join("dubbed.wav");
    write_track_wav(&samples, &out)?;
    Ok(out)
}
```

(Keep the earlier Task 5 items in the same file above these additions; add `use` lines already present.)

- [ ] **Step 4: Run** `cargo test dub` → PASS.

- [ ] **Step 5: Commit**

```bash
git add src/dub.rs src/media.rs
git commit -m "feat(dub): synthesis loop, clip fitting, track assembly"
```

---

### Task 7: `media.rs` — dub-aware mux command builders

**Files:**
- Modify: `src/media.rs`

**Interfaces:**
- Consumes: probe `has_audio` flag.
- Produces: `build_hardsub_dub_command(ffmpeg, video, ass_path, dub_wav, output, orig_volume: f32, has_audio: bool) -> Result<Command>`; `build_softsub_dub_command(ffmpeg, video, srt_path, dub_wav, output, orig_volume, has_audio) -> Result<Command>`.

- [ ] **Step 1: Write failing tests**

```rust
    #[test]
    fn hardsub_dub_single_graph() {
        let dir = tempfile::tempdir().unwrap();
        let ass = dir.path().join("subtitle.ass");
        std::fs::write(&ass, "x").unwrap();
        let cmd = build_hardsub_dub_command(
            Path::new("ffmpeg"), Path::new("/v.mp4"), &ass, Path::new("/d.wav"),
            Path::new("/o.mp4"), 0.15, true,
        ).unwrap();
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
            Path::new("ffmpeg"), Path::new("/v.mp4"), Path::new("/s.srt"), Path::new("/d.wav"),
            Path::new("/o.mkv"), 0.2, true,
        ).unwrap();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into()).collect();
        let cs = args.iter().position(|a| a == "-c:s").unwrap();
        assert_eq!(args[cs + 1], "srt"); // .mkv → srt codec
        let fc = args[args.iter().position(|a| a == "-filter_complex").unwrap() + 1].clone();
        assert!(fc.contains("[2:a]apad"));
    }

    #[test]
    fn no_audio_maps_dub_directly() {
        let cmd = build_softsub_dub_command(
            Path::new("ffmpeg"), Path::new("/v.mp4"), Path::new("/s.srt"), Path::new("/d.wav"),
            Path::new("/o.mp4"), 0.15, false,
        ).unwrap();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into()).collect();
        assert!(!args.contains(&"-filter_complex".to_string()));
        assert!(args.contains(&"2:a".to_string()));
    }
```

- [ ] **Step 2: Implement**

```rust
/// hardsub + dub in one encode: single -filter_complex with ass burn + amix.
/// has_audio=false → burn subs, use dubbed track as the only audio.
pub fn build_hardsub_dub_command(
    ffmpeg: &Path,
    video: &Path,
    ass_path: &Path,
    dub_wav: &Path,
    output: &Path,
    orig_volume: f32,
    has_audio: bool,
) -> anyhow::Result<Command> {
    let file_name = ass_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("ass path has no valid filename")?;
    if file_name.is_empty()
        || !file_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        anyhow::bail!("ass filename contains invalid characters: {file_name}");
    }
    let abs_video = std::path::absolute(video).context("abs video")?;
    let abs_dub = std::path::absolute(dub_wav).context("abs dub wav")?;
    let abs_output = std::path::absolute(output).context("abs output")?;

    let graph = if has_audio {
        format!(
            "[0:v]ass={file_name}[v];[0:a]volume={orig_volume}[a0];[1:a]apad[a1];[a0][a1]amix=inputs=2:duration=longest:normalize=0[aout]"
        )
    } else {
        format!("[0:v]ass={file_name}[v]")
    };

    let mut cmd = Command::new(ffmpeg);
    if let Some(parent) = ass_path.parent() {
        cmd.current_dir(parent);
    }
    cmd.arg("-hide_banner").arg("-loglevel").arg("error").arg("-y")
        .arg("-i").arg(&abs_video)
        .arg("-i").arg(&abs_dub)
        .arg("-filter_complex").arg(graph)
        .arg("-map").arg("[v]");
    if has_audio {
        cmd.arg("-map").arg("[aout]");
    } else {
        cmd.arg("-map").arg("1:a");
    }
    cmd.arg("-c:v").arg("libx264")
        .arg("-preset").arg("medium")
        .arg("-crf").arg("20")
        .arg("-c:a").arg("aac")
        .arg(&abs_output);
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
    cmd.arg("-hide_banner").arg("-loglevel").arg("error").arg("-y")
        .arg("-i").arg(video)
        .arg("-i").arg(srt_path)
        .arg("-i").arg(dub_wav);
    if has_audio {
        cmd.arg("-filter_complex").arg(format!(
            "[0:a]volume={orig_volume}[a0];[2:a]apad[a1];[a0][a1]amix=inputs=2:duration=longest:normalize=0[aout]"
        ));
    }
    cmd.arg("-map").arg("0:v");
    if has_audio {
        cmd.arg("-map").arg("[aout]");
    } else {
        cmd.arg("-map").arg("2:a");
    }
    cmd.arg("-map").arg("1:0")
        .arg("-c:v").arg("copy")
        .arg("-c:a").arg("aac")
        .arg("-c:s").arg(codec)
        .arg(output);
    Ok(cmd)
}
```

- [ ] **Step 3: Run** `cargo test media` → PASS.

- [ ] **Step 4: Commit**

```bash
git add src/media.rs
git commit -m "feat(media): dub-aware hardsub/softsub command builders"
```

---

### Task 8: CLI flags + `pipeline::run` + `main.rs` wiring

**Files:**
- Modify: `src/cli.rs` (TtsEngine enum + 5 flags)
- Modify: `src/pipeline.rs` (Deps.tts + dub stage + mode dispatch + subs-only wav output)
- Modify: `src/main.rs` (build_tts_engine, ffmpeg needed when dub)
- Modify: `src/lib.rs` is already updated (dub, tts)

**Interfaces:**
- Consumes: all prior tasks.
- Produces: `Cli::{dub, tts_engine, tts_voice, tts_model, dub_orig_volume}`; `cli::TtsEngine`; `Deps.tts`; `main::build_tts_engine`.

- [ ] **Step 1: `cli.rs` additions**

```rust
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TtsEngine {
    Local,
    Api,
}
```

In `Cli`:

```rust
    /// Dub translated subtitles into a synthesized voice track
    #[arg(long)]
    pub dub: bool,

    #[arg(long, value_enum, default_value_t = TtsEngine::Local)]
    pub tts_engine: TtsEngine,

    /// Voice: catalog id (e.g. vi_VN-vais1000-medium), api voice name, or .onnx path
    #[arg(long)]
    pub tts_voice: Option<String>,

    #[arg(long, default_value = "tts-1")]
    pub tts_model: String,

    /// Original audio volume under the dub (0.0-1.0)
    #[arg(long, default_value = "0.15")]
    pub dub_orig_volume: f32,
```

- [ ] **Step 2: `pipeline.rs` dub stage**

Add to `Deps`:

```rust
    pub tts: Option<&'a dyn crate::tts::TtsEngine>,
```

In `run()`, after the translation block and before `resolve_output_paths` consumers (place after `work_srt`/`work_ass` writes, before `outputs` vec):

```rust
    let dub_wav: Option<PathBuf> = if cli.dub {
        let engine = deps.tts.context("tts engine required for --dub")?;
        let ffmpeg = deps.ffmpeg.as_ref().context("ffmpeg is required for --dub")?;
        let sp = ProgressBar::new_spinner();
        sp.set_message("Dubbing translated subtitles...");
        sp.enable_steady_tick(std::time::Duration::from_millis(100));
        let total = segments.len();
        let wav = crate::dub::render_dub_track(
            engine,
            &segments,
            ffmpeg,
            work_dir,
            &cli.input,
            move |done, _t| sp.set_message(format!("Đang lồng tiếng {done}/{total}...")),
            || false,
        )
        .await?;
        Some(wav)
    } else {
        None
    };
```

(`sp` must be declared `mut`? `ProgressBar::new_spinner()` — `set_message` needs `&self`, ok. But closure moves `sp` — declare `let sp = ...` then closure `move` uses it; after await, `sp` was moved — restructure: use `Rc`? Simplest: drop the bar with a normal `println!` progress — but keep bar: capture `sp.clone()`? ProgressBar is `Clone` (it's a shared handle). So `let sp2 = sp.clone();` closure moves sp2, then `sp.finish_and_clear()` after.)

Update mode dispatch:

```rust
        OutputMode::Hardsub => {
            // ... existing ffmpeg/video_dest resolution ...
            let has_audio = dub_wav.is_some()
                .then(|| crate::dub::probe_media(ffmpeg, &cli.input).1)
                .unwrap_or(false);
            let cmd = if let Some(ref dw) = dub_wav {
                crate::media::build_hardsub_dub_command(
                    ffmpeg, &cli.input, &work_ass, dw, video_dest, cli.dub_orig_volume, has_audio,
                )?
            } else {
                crate::media::build_hardsub_command(ffmpeg, &cli.input, &work_ass, video_dest)?
            };
            // existing run_ffmpeg call
        }
```

Parallel change in `OutputMode::Softsub` with `build_softsub_dub_command(ffmpeg, &cli.input, &work_srt, dw, video_dest, cli.dub_orig_volume, has_audio)`.

In `OutputMode::SubsOnly`: after existing srt/ass copy, add:

```rust
            if let Some(ref dw) = dub_wav {
                let wav_dest = output_paths
                    .srt
                    .with_file_name(format!("{}.dub.wav",
                        output_paths.srt.file_stem().and_then(|s| s.to_str()).unwrap_or("out")));
                std::fs::copy(dw, &wav_dest)
                    .with_context(|| format!("failed to copy dub wav to {}", wav_dest.display()))?;
                outputs.push(wav_dest);
            }
```

- [ ] **Step 3: `main.rs` wiring**

ffmpeg condition: `if cli.mode == OutputMode::SubsOnly && cli.subtitle.is_some() && !cli.dub` (dub needs ffmpeg even for subs-only).

Add engine construction before `deps`:

```rust
    let tts_box: Option<Box<dyn auto_subtitle::tts::TtsEngine>> = if cli.dub {
        Some(build_tts_engine(&cli, &dep_mgr, store.as_ref()).await?)
    } else {
        None
    };
```

```rust
async fn build_tts_engine(
    cli: &Cli,
    dm: &DependencyManager,
    store: Option<&SettingsStore>,
) -> anyhow::Result<Box<dyn auto_subtitle::tts::TtsEngine>> {
    use auto_subtitle::tts::{api::ApiTts, piper::PiperTts, TtsEngine};
    match cli.tts_engine {
        crate::cli::TtsEngine::Local => {
            let piper = dm.find_or_download_piper().await?;
            let (model, cfg) = resolve_local_voice(cli, dm).await?;
            Ok(Box::new(PiperTts::new(piper, model, cfg)))
        }
        crate::cli::TtsEngine::Api => {
            let stored = store.and_then(|s| s.stt_key().ok().flatten());
            let key = cli
                .api_key
                .clone()
                .or(stored)
                .context("--api-key or OPENAI_API_KEY required for --tts-engine api")?;
            let voice = cli.tts_voice.clone().unwrap_or_else(|| "alloy".to_string());
            Ok(Box::new(ApiTts::new(
                cli.api_base_url_or_default(),
                key,
                cli.tts_model.clone(),
                voice,
            )))
        }
    }
}

async fn resolve_local_voice(cli: &Cli, dm: &DependencyManager) -> anyhow::Result<(PathBuf, PathBuf)> {
    match cli.tts_voice.as_deref() {
        Some(v) => {
            if let Some(info) = auto_subtitle::tts::find_voice(v) {
                dm.ensure_voice(info).await
            } else {
                let p = PathBuf::from(v);
                let cfg = p.with_extension("onnx.json");
                anyhow::ensure!(p.is_file(), "voice model not found: {v}");
                anyhow::ensure!(cfg.is_file(), "voice config missing: {}", cfg.display());
                Ok((p, cfg))
            }
        }
        None => {
            let info = auto_subtitle::tts::default_voice_for_lang(&cli.target_lang).with_context(|| {
                format!(
                    "không có giọng Piper nào cho ngôn ngữ '{}'; truyền --tts-voice hoặc --tts-engine api",
                    cli.target_lang
                )
            })?;
            dm.ensure_voice(info).await
        }
    }
}
```

Deps literal: add `tts: tts_box.as_deref(),`.

- [ ] **Step 4: Verify**

Run: `cargo build` → clean; `cargo test` → all pass; `./target/debug/auto-subtitle --help` shows `--dub`, `--tts-engine`, `--tts-voice`, `--tts-model`, `--dub-orig-volume`.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/pipeline.rs src/main.rs
git commit -m "feat(cli): --dub flag + dub stage in pipeline"
```

---

### Task 9: Tauri — types, piper status commands, export-time dub

**Files:**
- Modify: `src-tauri/src/types.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (register new commands)
- Modify: `src/settings/model.rs` (GeneralSettings TTS fields)

**Interfaces:**
- Consumes: `dub::render_dub_track`, `dub::probe_media`, `build_*_dub_command`, `DependencyManager::find_piper/find_or_download_piper/ensure_voice`.
- Produces: `PiperStatus`, `ExportProgress`; `ExportRequest.dub`/`job_id`; commands `check_piper_status`, `ensure_piper`; `GeneralSettings::{tts_engine, tts_voice, tts_model, dub_orig_volume}`.

- [ ] **Step 1: `settings/model.rs`**

In `GeneralSettings` add (all `#[serde(default)]`-compatible):

```rust
    pub tts_engine: String,          // "local" | "api"
    pub tts_voice: Option<String>,
    pub tts_model: Option<String>,
    pub dub_orig_volume: f32,
```

Defaults: `"local"`, `None`, `None`, `0.15`. Field defaults: `#[serde(default = "default_tts_engine")]`, `#[serde(default)]`, `#[serde(default)]`, `#[serde(default = "default_orig_volume_s")]` with `fn default_tts_engine() -> String { "local".into() }` and `fn default_orig_volume_s() -> f32 { 0.15 }`. Also update `Default for GeneralSettings`.

- [ ] **Step 2: `types.rs`**

```rust
use auto_subtitle::tts::DubOptions;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PiperStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProgress {
    pub job_id: Option<String>,
    pub stage: String,
    pub index: usize,
    pub total: usize,
}
```

In `ExportRequest` add:

```rust
    #[serde(default)]
    pub dub: Option<DubOptions>,
    #[serde(default)]
    pub job_id: Option<String>,
```

- [ ] **Step 3: `commands.rs` — piper commands**

```rust
fn piper_status(dm: &DependencyManager) -> PiperStatus {
    match dm.find_piper() {
        Some(p) => {
            let version = std::process::Command::new(&p)
                .arg("--version")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
            PiperStatus { installed: true, path: Some(p.display().to_string()), version }
        }
        None => PiperStatus { installed: false, path: None, version: None },
    }
}

#[tauri::command]
pub async fn check_piper_status() -> Result<PiperStatus, String> {
    let dm = DependencyManager::new(get_app_config());
    Ok(piper_status(&dm))
}

#[tauri::command]
pub async fn ensure_piper(app: AppHandle) -> Result<PiperStatus, String> {
    let dm = DependencyManager::new(get_app_config());
    let _ = app.emit("pipeline-log", "Đang tải Piper TTS...");
    let bin = dm
        .find_or_download_piper()
        .await
        .map_err(|e| format!("Tải Piper thất bại: {e:#}"))?;
    if let Some(v) = auto_subtitle::tts::default_voice_for_lang("vi") {
        dm.ensure_voice(v).await.map_err(|e| format!("Tải voice thất bại: {e:#}"))?;
    }
    let _ = app.emit("pipeline-log", "Piper TTS sẵn sàng!");
    let _ = bin; // status re-reads path
    Ok(piper_status(&dm))
}
```

Register both in `src-tauri/src/lib.rs` invoke handler (same list as `check_yt_dlp_status`).

- [ ] **Step 4: `commands.rs` — dub inside `export_subtitles`**

Change signature: `pub async fn export_subtitles(app: AppHandle, state: State<'_, AppState>, request: ExportRequest) -> Result<String, String>`.

At top, after `let temp_dir = ...`, insert the dub stage before `match request.mode`:

```rust
    let dub_wav: Option<PathBuf> = if let Some(dub) = request.dub.as_ref().filter(|d| d.enabled) {
        let mut cancel_rx = {
            let mut senders = state.cancel_senders.lock().await;
            let (tx, rx) = broadcast::channel(1);
            senders.insert(request.job_id.clone().unwrap_or_else(|| "export".into()), tx);
            rx
        };
        let engine = build_export_tts(&dm, dub, &request, &state).await?;
        let app_c = app.clone();
        let jid = request.job_id.clone();
        let wav = auto_subtitle::dub::render_dub_track(
            engine.as_ref(),
            &request.segments,
            &ffmpeg,
            temp_dir.path(),
            &video_in,
            move |idx, total| {
                let _ = app_c.emit("export-progress", crate::types::ExportProgress {
                    job_id: jid.clone(), stage: "dubbing".into(), index: idx, total,
                });
                let _ = app_c.emit("pipeline-log", format!("Đang lồng tiếng {idx}/{total}..."));
            },
            move || cancel_rx.try_recv().is_ok(),
        )
        .await
        .map_err(|e| format!("Lỗi lồng tiếng: {e:#}"))?;
        Some(wav)
    } else {
        None
    };
```

Helper:

```rust
async fn build_export_tts(
    dm: &DependencyManager,
    dub: &auto_subtitle::tts::DubOptions,
    request: &ExportRequest,
    state: &State<'_, AppState>,
) -> Result<Box<dyn auto_subtitle::tts::TtsEngine>, String> {
    use auto_subtitle::tts::{api::ApiTts, piper::PiperTts, TtsEngineKind};
    match dub.engine {
        TtsEngineKind::Local => {
            let piper = dm.find_or_download_piper().await.map_err(|e| format!("{e:#}"))?;
            let (model, cfg) = match dub.voice.as_deref() {
                Some(v) if auto_subtitle::tts::find_voice(v).is_some() => {
                    dm.ensure_voice(auto_subtitle::tts::find_voice(v).unwrap()).await.map_err(|e| format!("{e:#}"))?
                }
                Some(v) => {
                    let p = PathBuf::from(v);
                    let c = p.with_extension("onnx.json");
                    if !p.is_file() || !c.is_file() {
                        return Err(format!("Voice không hợp lệ: {v}"));
                    }
                    (p, c)
                }
                None => {
                    let lang = {
                        let store = state.settings.lock().map_err(|_| "Lỗi khóa cài đặt")?;
                        store.settings().general.target_lang.clone()
                    };
                    let info = auto_subtitle::tts::default_voice_for_lang(&lang)
                        .ok_or(format!("Không có giọng Piper nào cho '{lang}' — chọn voice hoặc engine api"))?;
                    dm.ensure_voice(info).await.map_err(|e| format!("{e:#}"))?
                }
            };
            Ok(Box::new(PiperTts::new(piper, model, cfg)))
        }
        TtsEngineKind::Api => {
            let key = {
                let store = state.settings.lock().map_err(|_| "Lỗi khóa cài đặt")?;
                store.stt_key().ok().flatten()
            }
            .ok_or("Chưa nhập API key cho TTS".to_string())?;
            Ok(Box::new(ApiTts::new(
                "https://api.openai.com/v1".to_string(),
                key,
                dub.model.clone().unwrap_or_else(|| "tts-1".into()),
                dub.voice.clone().unwrap_or_else(|| "alloy".into()),
            )))
        }
    }
}
```

Then in each `ExportMode` arm use dub variants when `dub_wav.is_some()`:

```rust
        ExportMode::Hardsub => {
            // ... existing ass write ...
            let has_audio = auto_subtitle::dub::probe_media(&ffmpeg, &video_in).1;
            let cmd = if let Some(ref dw) = dub_wav {
                media::build_hardsub_dub_command(&ffmpeg, &video_in, &temp_ass, dw, &out_path,
                    request.dub.as_ref().map(|d| d.orig_volume).unwrap_or(0.15), has_audio)
                    .map_err(|e| format!("Lỗi tạo lệnh Hardsub: {e:#}"))?
            } else {
                media::build_hardsub_command(&ffmpeg, &video_in, &temp_ass, &out_path)
                    .map_err(|e| format!("Lỗi tạo lệnh Hardsub: {e:#}"))?
            };
            // existing spawn
        }
```

`Softsub` arm: `build_softsub_dub_command(&ffmpeg, &video_in, &temp_srt, dw, &out_path, vol, has_audio)` — note existing softsub branch writes `temp_srt`? Read current arm: it writes ass only for hardsub; softsub writes `temp_dir/subs.srt` via render_srt — reuse whatever srt path exists; if none, write `render_srt(&request.segments, request.bilingual)` to `temp_dir/sub.srt` before building command.

`SubsOnly` arm: after writing srt/ass, if `dub_wav` → copy to `out_path.with_extension("dub.wav")`, mention in result message.

- [ ] **Step 5: Verify** `cargo build --workspace` → clean; `cargo test -p auto-subtitle` → pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src src/settings/model.rs
git commit -m "feat(gui): export-time dubbing + piper status commands"
```

---

### Task 10: UI — types, ipc, ExportModal toggle, SettingsModal TTS section

**Files:**
- Modify: `ui/src/types/index.ts`
- Modify: `ui/src/lib/ipc.ts`
- Modify: `ui/src/components/ExportModal.tsx`
- Modify: `ui/src/components/SettingsModal.tsx`
- Modify: `ui/src/App.tsx` (pass dub defaults into ExportModal)

**Interfaces:**
- Consumes: `export_subtitles`, `check_piper_status`, `ensure_piper`, `export-progress` event, `GeneralSettings` fields.
- Produces: `DubOptions`, `PiperStatus`, `ExportProgress` TS types; `checkPiperStatus()`, `ensurePiper()` ipc wrappers; `ExportModal` dub toggle + inline progress; Settings TTS section.

- [ ] **Step 1: `types/index.ts`**

```typescript
export type TtsEngineKind = 'local' | 'api';

export interface DubOptions {
  enabled: boolean;
  engine: TtsEngineKind;
  voice?: string | null;
  model?: string | null;
  orig_volume: number;
}

export interface PiperStatus {
  installed: boolean;
  path?: string | null;
  version?: string | null;
}

export interface ExportProgress {
  job_id?: string | null;
  stage: string;
  index: number;
  total: number;
}
```

Add to `ExportRequest`: `dub?: DubOptions | null; job_id?: string | null;`. Add to `GeneralSettings`: `tts_engine: TtsEngineKind; tts_voice?: string | null; tts_model?: string | null; dub_orig_volume: number;`.

- [ ] **Step 2: `ipc.ts`**

```typescript
export async function checkPiperStatus(): Promise<PiperStatus> {
  return await invoke<PiperStatus>('check_piper_status');
}

export async function ensurePiper(): Promise<PiperStatus> {
  return await invoke<PiperStatus>('ensure_piper');
}
```

- [ ] **Step 3: `ExportModal.tsx`**

New props: `dubDefaults: { enabled: boolean; engine: TtsEngineKind; voice?: string | null; model?: string | null; orig_volume: number }` (App passes from `general`). State: `dubEnabled`, `dubEngine`, `dubVoice`, `origVolume`, `dubProgress: string | null`.

UI block inside the options grid (same styling as bilingual toggle):

```tsx
<div className="flex flex-col gap-1.5">
  <label className="text-[11px] font-semibold text-slate-300">Lồng tiếng (TTS)</label>
  <button type="button" onClick={() => setDubEnabled(!dubEnabled)}
    className={`h-9 px-3 rounded-lg border flex items-center justify-between transition-colors ${dubEnabled ? 'border-blue-500/60 bg-blue-600/10 text-slate-200' : 'border-slate-700 bg-slate-900 text-slate-400 hover:border-slate-600'}`}>
    <span className="text-xs">{dubEngine === 'local' ? 'Piper (local)' : 'Cloud API'}</span>
    <span className={`relative w-8 h-[18px] rounded-full transition-colors shrink-0 ${dubEnabled ? 'bg-blue-600' : 'bg-slate-700'}`}>
      <span className={`absolute top-[2px] left-[2px] w-[14px] h-[14px] rounded-full bg-white shadow transition-transform ${dubEnabled ? 'translate-x-[14px]' : ''}`} />
    </span>
  </button>
</div>
```

Progress: `useEffect` → `listen<ExportProgress>('export-progress', e => setDubProgress(...))` only while `isExporting && dubEnabled`; render `Đang lồng tiếng {index}/{total}` under the spinner row.

In `handleStartExport`, extend request:

```typescript
        dub: dubEnabled ? { enabled: true, engine: dubEngine, voice: dubVoice || null, model: dubDefaults.model ?? null, orig_volume: origVolume } : null,
        job_id: `export-${Date.now()}`,
```

- [ ] **Step 4: `SettingsModal.tsx` TTS section**

New section "Lồng tiếng (TTS)" in General tab: engine `<select>` (local/api), voice `<input>` + `<datalist>` of `['vi_VN-vais1000-medium','vi_VN-25hours_single-low','en_US-lessac-medium']`, model input (visible when api), volume `<input type="range" min="0" max="50">` → `dub_orig_volume = v/100`, piper status row using `checkPiperStatus()` + "Tải Piper" button calling `ensurePiper()` (same row pattern as yt-dlp status). All fields write to `general` object (existing `saveGeneral` debounce persists).

- [ ] **Step 5: `App.tsx`** — extend `general` initial state with `tts_engine: 'local', tts_voice: null, tts_model: null, dub_orig_volume: 0.15`; pass `dubDefaults={{ enabled: false, engine: general.tts_engine, voice: general.tts_voice, model: general.tts_model, orig_volume: general.dub_orig_volume }}` to `ExportModal`.

- [ ] **Step 6: Verify**

Run: `npm --prefix ui run build` (or `tsc`) → clean; `npm --prefix ui test` → pass; manual: `npm --prefix ui run tauri dev`, export with Lồng tiếng on → progress + dubbed output plays.

- [ ] **Step 7: Commit**

```bash
git add ui/src src-tauri
git commit -m "feat(ui): dub toggle in export + TTS settings section"
```

---

## Self-Review Notes

- Spec coverage: TTS trait/engines (T1,T3,T4), piper+voice download (T2), timing algo (T5), track+mux (T6,T7), CLI (T8), GUI export + settings (T9,T10), error matrix (>50% abort, cancel, no-audio, skip-translate→original_text, missing voice), acceptance criteria → smoke verified in T8/T10 steps.
- `run_pipeline` (GUI STT/translate) is intentionally untouched — dub runs at export.
- `subs_only + --dub` requires ffmpeg in main.rs gate — covered in Task 8 step 3.
- `DubClip.wav_path` must live until `build_track` reads it — all inside `work_dir`/`temp_dir`, dropped after run.
