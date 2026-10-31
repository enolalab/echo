# Auto-Subtitle (Rust)

A fast, modular CLI tool written in Rust to automatically generate, translate, and burn/mux subtitles into video files using local AI (Whisper + Ollama) or Cloud APIs.

## Features

- **Speech-to-Text (STT):** Local `whisper.cpp` (via `whisper-rs`) with CPU/CUDA support, or Cloud API fallback (OpenAI/Groq).
- **Translation:** Local LLM via Ollama (`qwen2.5:7b` by default) with context-aware batching, or OpenAI-compatible cloud endpoints.
- **Existing Subtitles:** Bypass STT by providing `.srt`, `.vtt`, or `.ass` directly.
- **Flexible Modes:** Hardsub (burn into video), Softsub (mux track), or Subs-only (`.srt` / `.ass`).
- **Bilingual Subtitles:** Display original and translated subtitles simultaneously.
- **Zero-Config UX:** Automatically downloads static FFmpeg and GGML Whisper models to `~/.cache/auto-subtitle/` if missing.

## Prerequisites

- **Rust:** Rust 2021 Edition (`cargo`, `rustc` 1.80+)
- **C/C++ Toolchain:** `cmake`, `clang`, `libclang-dev`, `pkg-config` (required for compiling `whisper.cpp`)
- **GPU (Optional):** NVIDIA GPU with CUDA Toolkit (`nvcc`) for `--features cuda`
- **Ollama (Optional for local translation):** `curl -fsSL https://ollama.com/install.sh | sh && ollama pull qwen2.5:7b`

## Build

```bash
# CPU build (default)
cargo build --release

# GPU CUDA build
cargo build --release --features cuda
```

## Usage Examples

```bash
# Basic: Transcribe with local Whisper + translate to Vietnamese with Ollama + Hardsub
auto-subtitle -i video.mp4

# Bilingual subtitles (original above, translated below)
auto-subtitle -i video.mp4 --bilingual

# Export subtitle files only (.srt and .ass), skip video encoding
auto-subtitle -i video.mp4 --mode subs-only

# Softsub (embed track into container without re-encoding video)
auto-subtitle -i video.mp4 --mode softsub

# Use existing subtitle file, bypass STT
auto-subtitle -i video.mp4 --sub existing.srt

# Use existing subtitle file and skip translation (only burn/mux into video)
auto-subtitle -i video.mp4 --sub existing.srt --skip-translate

# Cloud API mode (OpenAI / Groq / DeepSeek)
auto-subtitle -i video.mp4 \
  --stt-engine api \
  --translate-engine openai-compat \
  --api-key "sk-..." \
  --api-model "deepseek-chat"
```
