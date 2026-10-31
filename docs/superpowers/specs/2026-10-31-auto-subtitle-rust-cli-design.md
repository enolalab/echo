# Design Document: Auto-Subtitle Rust CLI

- **Date:** 2026-10-31
- **Project:** `auto-subtitle`
- **Language / Runtime:** Rust (2021 Edition)
- **Status:** Approved by User

---

## 1. Executive Summary

`auto-subtitle` is a fast, robust, and modular command-line tool written in Rust that automates the end-to-end workflow of generating, translating, and embedding subtitles into video files.

The tool operates on a **Hybrid Model**:
- **Default (Local & Offline):** Runs speech recognition locally via `whisper-rs` (with NVIDIA CUDA GPU acceleration) and translates subtitles locally via `Ollama` (using models like `qwen2.5:7b`).
- **Cloud Fallback / API Mode:** Supports OpenAI-compatible REST endpoints for STT (Whisper API / Groq) and LLM translation (DeepSeek, OpenAI, etc.).
- **Zero-Config UX:** Automatically downloads missing static FFmpeg binaries and GGML Whisper model weights on demand to user cache directories.
- **Pre-existing Subtitle Support:** Accepts existing subtitle files (`.srt`, `.vtt`, `.ass`) via `--subtitle`, skipping STT extraction entirely to directly translate and/or burn into video.

---

## 2. System Architecture

The project follows a **Trait-based Modular Architecture**, allowing components to be tested and swapped independently without coupling.

```
                    ┌─────────────────────────┐
                    │      Input Video        │
                    └────────────┬────────────┘
                                 │
           ┌─────────────────────┴─────────────────────┐
           │                                           │
  (No external sub)                            (With --subtitle file)
           ▼                                           ▼
[Audio Extractor (FFmpeg)]                    [Subtitle Parser (.srt/.vtt)]
           │                                           │
           ▼ (16kHz mono WAV)                          │
[Transcriber (whisper-rs)]                             │
           │                                           │
           └─────────────────────┬─────────────────────┘
                                 ▼ (Vec<SubtitleSegment>)
                     [Translator (Ollama / API)]  <── (Skipped if --skip-translate)
                                 │
                                 ▼ (Translated SubtitleSegments)
                     [Subtitle Engine (.srt / .ass)]
                                 │
                                 ▼
                    [Video Burner / Muxer (FFmpeg)]
                                 │
                                 ▼
                    ┌─────────────────────────┐
                    │   Output Subtitled Video│
                    └─────────────────────────┘
```

### 2.1 File & Module Layout

```text
auto-subtitle/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entry point, argument parsing, pipeline orchestration
│   ├── config.rs            # Config & environment settings (cache dirs, API keys)
│   ├── downloader.rs        # Auto-download for static FFmpeg & Whisper GGML models
│   │
│   ├── audio/
│   │   ├── mod.rs
│   │   └── extractor.rs     # Extract 16kHz mono WAV from video via FFmpeg
│   │
│   ├── stt/
│   │   ├── mod.rs           # Transcriber trait definition
│   │   ├── whisper_local.rs # whisper-rs implementation with CUDA / CPU fallback
│   │   └── whisper_api.rs   # Cloud API transcription fallback (OpenAI/Groq)
│   │
│   ├── translate/
│   │   ├── mod.rs           # Translator trait definition
│   │   ├── ollama.rs        # Ollama local LLM translation (batching)
│   │   └── openai_compat.rs # OpenAI-compatible cloud LLM translation
│   │
│   ├── subtitle/
│   │   ├── mod.rs
│   │   ├── model.rs         # SubtitleSegment & SubtitleTrack data structs
│   │   ├── parser.rs        # Parse existing .srt / .vtt / .ass files into SubtitleSegment
│   │   ├── srt.rs           # SRT formatting & generation
│   │   └── ass.rs           # ASS formatting (styling, colors, bilingual layout)
│   │
│   └── video/
│       ├── mod.rs
│       └── burner.rs        # Hardsub (burn-in) and Softsub (mux track) via FFmpeg
└── tests/
    ├── parser_test.rs
    ├── srt_test.rs
    ├── ass_test.rs
    └── translator_test.rs
```

---

## 3. Data Models & Core Traits

### 3.1 Subtitle Representation

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleSegment {
    pub id: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    pub original_text: String,
    pub translated_text: Option<String>,
}
```

### 3.2 Transcriber Trait

```rust
use async_trait::async_trait;
use std::path::Path;
use anyhow::Result;

#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(&self, audio_path: &Path) -> Result<Vec<SubtitleSegment>>;
}
```

### 3.3 Translator Trait (Batching for Context Awareness)

```rust
#[async_trait]
pub trait Translator: Send + Sync {
    /// Translates a batch of subtitle segments to preserve narrative context and pronouns.
    async fn translate_batch(
        &self,
        segments: &[SubtitleSegment],
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<String>>;
}
```

---

## 4. CLI Interface & Parameters

Built with `clap` derive mode:

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--input <PATH>` | `-i` | *(Required)* | Input video file path. |
| `--output <PATH>` | `-o` | `<stem>_subtitled.mp4` | Output video file path or directory. |
| `--subtitle <PATH>` | `--sub` | None | Optional path to existing subtitle file (`.srt`, `.vtt`, `.ass`). If provided, bypasses audio extraction & STT. |
| `--skip-translate` | | `false` | Skip translation step (useful when input subtitle is already translated and only needs burning/muxing). |
| `--source-lang <LANG>` | `-s` | `auto` | Source language code (`auto` for Whisper auto-detection). |
| `--target-lang <LANG>` | `-t` | `vi` | Target translation language (e.g. `vi`, `en`, `ja`). |
| `--mode <MODE>` | `-m` | `hardsub` | Output mode: `hardsub` (burn-in), `softsub` (mux track), `subs-only` (export .srt/.ass only). |
| `--bilingual` | `-b` | `false` | If enabled, shows both original and translated text in ASS/SRT subtitles. |
| `--stt-engine <ENG>` | | `local` | `local` (whisper.cpp) or `api` (OpenAI/Groq). |
| `--whisper-model <M>` | | `small` | GGML model size: `tiny`, `base`, `small`, `medium`, `large-v3`. |
| `--translate-engine <ENG>` | | `ollama` | `ollama` (local) or `openai-compat` (cloud). |
| `--ollama-model <M>` | | `qwen2.5:7b` | Model name deployed in Ollama. |
| `--ollama-url <URL>` | | `http://localhost:11434` | Ollama service base URL. |
| `--api-key <KEY>` | | None | API Key for cloud STT / Translation (can also read from env `OPENAI_API_KEY`). |
| `--api-base-url <URL>` | | `https://api.openai.com/v1` | Base URL for OpenAI-compatible STT / translation endpoints. |
| `--api-model <M>` | | `gpt-4o-mini` | Chat model used by `openai-compat` translation. |
| `--stt-api-model <M>` | | `whisper-1` | Model used by `--stt-engine api`. |
| `--keep-temp` | | `false` | Keep intermediate `.wav` and `.srt/.ass` files on completion. |

---

## 5. Dependency Management & Zero-Config UX

1. **FFmpeg Discovery & Auto-Download:**
   - Check if `ffmpeg` exists in system `PATH`.
   - If not found, download a static build of FFmpeg from John Van Sickle's static releases (`ffmpeg-release-amd64-static.tar.xz`) to `~/.cache/auto-subtitle/bin/ffmpeg`.
   - Mark as executable (`chmod +x`) and use directly.

2. **Whisper Model Auto-Download:**
   - Cache directory: `~/.cache/auto-subtitle/models/`.
   - Download target: `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{model}.bin`.
   - Displays a progress bar using `indicatif` during download.

---

## 6. Error Handling & Resilience

1. **Ollama Availability:**
   - Send health check `GET http://localhost:11434/api/tags` prior to processing.
   - If unreachable, emit an actionable user prompt advising how to start Ollama or switch to `--translate-engine openai-compat`.
2. **GPU Memory Allocation:**
   - Detect CUDA device availability. If GPU VRAM fails or CUDA initialization fails, log a warning and fall back to multi-threaded CPU inference.
3. **Subtitle Timecode Sanitization:**
   - Enforce invariant `start_ms < end_ms`.
   - Collapse zero-duration or overlapping segments.
   - Sanitize ASS override tags to prevent formatting corruption.

---

## 7. Testing Strategy

1. **Unit Testing:**
   - `test_subtitle_parser`: Verifies parsing of `.srt`, `.vtt`, and `.ass` files into structured `SubtitleSegment` objects, checking accurate millisecond conversion and multiline handling.
   - `test_srt_formatting`: Verifies timestamps conform to `00:00:00,000 --> 00:00:00,000` and line breaks match SRT specification.
   - `test_ass_bilingual_styling`: Verifies ASS header, styles (FontName, PrimaryColour, SecondaryColour), and layered layout for bilingual subtitles.
   - `test_translation_batch_parsing`: Verifies JSON/indexed line responses from LLM accurately map 1:1 back to segments.
2. **Integration Testing:**
   - End-to-end execution on a synthetic 3-second audio/video clip using mock transcriber and mock translator to verify correct video file output generation.
