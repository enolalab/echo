# Design Document: Echo Desktop UI (Tauri + React)

- **Date:** 2026-11-01
- **Project:** `auto-subtitle` (Echo)
- **Tech Stack:** Tauri v2, Rust (2021 Edition), React 18+, TypeScript, Tailwind CSS, Vite
- **Status:** Approved by User

---

## 1. Executive Summary

Echo (`auto-subtitle`) is currently a CLI tool written in Rust that automates speech-to-text (STT via Whisper), context-aware subtitle translation (via Ollama/OpenAI API), and subtitle embedding (hardsub/softsub via FFmpeg).

This design introduces a **cross-platform Desktop GUI** built with **Tauri v2** and **React + Tailwind CSS** following a **Studio All-in-One** workflow. The UI allows users to:
1. **One-click Pipeline:** Drag-and-drop video, auto-detect local hardware/models, execute end-to-end transcription & translation with live progress indicators.
2. **Interactive Subtitle Editor:** Bidirectionally synchronize video playback with an editable subtitle table (click subtitle line to seek video; video playback highlights active subtitle).
3. **Visual Customization & Export:** Select high-contrast subtitle presets (*YouTube Yellow*, *Classic White*, *Netflix Modern*, *Bilingual Highlight*) and export to Hardsub video, Softsub video, or standalone `.srt`/`.ass` subtitle files.
4. **Preserve CLI:** Maintain 100% independence of the existing CLI binary through a shared core library crate.

---

## 2. System Architecture & Code Organization

The repository is structured as a Cargo workspace with separation between core domain logic, CLI binary, and the Tauri desktop application.

```
auto-subtitle/
├── Cargo.toml                       # Top-level workspace definition
├── src/                             # Core library & CLI binary
│   ├── lib.rs                       # Exported core modules (pipeline, stt, translate, media, downloader)
│   ├── main.rs                      # Existing CLI entry point (thin wrapper around core)
│   ├── pipeline.rs                  # Core pipeline orchestrator
│   ├── downloader.rs                # Static FFmpeg & Whisper GGML downloader
│   ├── media.rs                     # Audio extraction, probe & muxing
│   ├── stt/                         # Whisper local & cloud engines
│   ├── translate/                   # Ollama local & cloud translation
│   └── subtitle/                    # SRT & ASS formatting and parsing
├── src-tauri/                       # Tauri v2 Desktop Backend (Rust)
│   ├── Cargo.toml                   # Tauri dependencies + auto-subtitle core lib
│   ├── tauri.conf.json              # Window settings, permissions, custom protocol
│   ├── src/
│   │   ├── main.rs                  # Tauri runtime setup & plugin registration
│   │   ├── commands.rs              # Tauri IPC commands (invoke handlers)
│   │   ├── state.rs                 # Global state (cancellation tokens, task tracker)
│   │   └── error.rs                 # Serialized error types for IPC
└── ui/                              # Frontend (React + TypeScript + Vite)
    ├── package.json
    ├── vite.config.ts
    ├── tailwind.config.js
    ├── index.html
    └── src/
        ├── App.tsx                  # Main Studio All-in-One layout
        ├── components/
        │   ├── HeaderBar.tsx        # Video picker, model selectors, action buttons
        │   ├── VideoPlayer.tsx      # Video canvas/player + realtime subtitle overlay
        │   ├── SubtitleEditor.tsx   # Editable subtitle table / virtualized list
        │   ├── ProgressDrawer.tsx   # Stage badges, percentage progress, log terminal
        │   └── ExportModal.tsx      # Hardsub/Softsub/Subs-only modal
        ├── hooks/
        │   ├── useSystemInfo.ts     # Poll/fetch GPU, Whisper models & Ollama models
        │   ├── usePipeline.ts       # IPC invocation & cancellation
        │   └── useSubtitleSync.ts   # Video timecode ⇄ Subtitle row active sync
        └── types/
            └── index.ts             # TypeScript definitions matching Rust Serde types
```

---

## 3. UI Layout & Component Breakdown (Studio All-in-One)

The interface follows the **Studio All-in-One** layout:

```
+---------------------------------------------------------------------------------------+
| [TOP BAR] 📁 input.mp4 | 🎙️ Whisper: base (✓ Downloaded) | 🌐 Ollama: qwen2.5:7b (VI)  |
|            🎨 Style: YouTube Yellow | 🔤 Bilingual: [On]     [ ▶ Start Auto-Sub ]     |
+-------------------------------------------+-------------------------------------------+
| [VIDEO PLAYER PREVIEW]                    | [SUBTITLE EDITOR TABLE]                   |
| +---------------------------------------+ | 🔍 [Search & Replace...        ] [+ Add]  |
| |                                       | | ----------------------------------------- |
| |                                       | | [#1] ⏱️ 00:00:01,200 → 00:00:04,500       |
| |                                       | | EN: Good morning everyone                 |
| |               [ ▶ ]                   | | VI: [Chào buổi sáng mọi người         ]  |
| |                                       | | ----------------------------------------- |
| |      "Chào buổi sáng mọi người"       | | [#2 - Active] ⏱️ 00:00:05,000 → 00:08    |
| +---------------------------------------+ | EN: Today we will talk about Echo Rust    |
| [▶] [00:05 / 03:40] ━━━━●━━━━━━ [1.0x] [🔁] | VI: [Hôm nay chúng ta nói về Echo Rust  ]  |
+-------------------------------------------+-------------------------------------------+
| [BOTTOM PROGRESS & LOG DRAWER]                                                        |
| [Extract Audio ✓] ➔ [Whisper STT ✓] ➔ [Ollama Translate 75%] ➔ [Ready]  [🎬 EXPORT]  |
+---------------------------------------------------------------------------------------+
```

### 3.1 HeaderBar
- **File Picker:** Supports drag-and-drop or system file dialog to select `.mp4`, `.mkv`, `.mov`, `.flac`, `.wav`.
- **Whisper Selector:** Dropdown of available models (`tiny`, `base`, `small`, `medium`, `large-v3`) with badges (`Downloaded` / `Not downloaded`). 1-click download with progress bar.
- **Translation Selector:** Auto-fetches local Ollama models (`qwen2.5:7b`, `llama3.2`, etc.) or switches to Cloud API (OpenAI / DeepSeek / Groq) with API key dialog.
- **Target Language & Bilingual Toggle:** Select destination language (Vietnamese, English, Japanese, etc.) and toggle bilingual subtitle display.
- **Preset Selector:** Quick style choices (*Classic White*, *YouTube Yellow*, *Netflix Modern*, *Bilingual Highlight*).
- **Run Button:** Triggers execution.

### 3.2 VideoPlayer
- Rendered using HTML5 `<video>` powered by Tauri's custom asset streaming protocol (`convertFileSrc(filePath)`), enabling smooth seeking on multi-gigabyte video files.
- Custom Subtitle Overlay layer that applies selected preset styles on top of the video canvas in real-time.
- Controls: Play/Pause, Timeline slider, Timecode display, Playback speed (`1x`, `1.25x`, `1.5x`, `2x`), Repeat current line shortcut (`R`).

### 3.3 SubtitleEditor
- **Bidirectional Sync:**
  - Video time updates ➔ active subtitle segment auto-highlights and scrolls into view.
  - Clicking any segment row ➔ video player immediately seeks to segment `start_ms`.
- **Inline Editing:** Direct editing of `translated_text` and `original_text`.
- **Row Actions:** Split line at cursor, Merge with next line, Delete line, Timecode nudging (`±0.1s`).
- **Batch Tools:** Global Search & Replace bar.
- **Performance:** Virtualized row rendering (`@tanstack/react-virtual`) ensuring 60fps scrolling for files with 1,000+ segments.

### 3.4 ProgressDrawer & ExportModal
- Visual stage badges: `Extract Audio` ➔ `Whisper STT` ➔ `LLM Translation` ➔ `Ready`.
- Segment progress indicator (`18/45 segments translated`) with live console log toggle.
- Export Modal triggers:
  - **Hardsub:** Burn-in subtitles via FFmpeg with libass filter.
  - **Softsub:** Embed subtitle stream into container (`mov_text` for MP4, `srt` for MKV) without re-encoding video.
  - **Subs-only:** Export `.srt` and `.ass` files directly.

---

## 4. IPC Interface & Data Contracts

Communication between React and Rust uses Tauri commands (`invoke`) and event streams (`listen`).

### 4.1 Data Models (TypeScript & Rust Serde)

```typescript
export interface SubtitleSegment {
  id: number;
  start_ms: u64;
  end_ms: u64;
  original_text: string;
  translated_text: string;
}

export interface SystemInfo {
  has_cuda: boolean;
  ollama_running: boolean;
  ollama_models: string[];
  whisper_models: {
    name: string;
    downloaded: boolean;
    size_mb: number;
  }[];
}

export interface PipelineConfig {
  video_path: string;
  subtitle_path?: string;
  whisper_model: string;
  use_cloud_stt: boolean;
  stt_api_key?: string;
  target_lang: string;
  skip_translate: boolean;
  use_cloud_llm: boolean;
  llm_model: string;
  llm_api_base?: string;
  llm_api_key?: string;
}

export interface ExportRequest {
  video_path: string;
  output_path: string;
  mode: 'hardsub' | 'softsub' | 'subs-only';
  bilingual: boolean;
  style_preset: 'classic_white' | 'youtube_yellow' | 'netflix_modern' | 'bilingual_highlight';
  segments: SubtitleSegment[];
}
```

### 4.2 Tauri Commands (`commands.rs`)

1. `get_system_info() -> Result<SystemInfo, String>`
2. `download_whisper_model(model_name: String) -> Result<(), String>`
3. `start_pipeline(config: PipelineConfig) -> Result<Vec<SubtitleSegment>, String>`
4. `cancel_pipeline() -> Result<(), String>`
5. `export_subtitles(request: ExportRequest) -> Result<String, String>`
6. `open_in_file_manager(path: String) -> Result<(), String>`

### 4.3 Real-time Events (Rust ➔ Frontend)

- `pipeline-stage`: `{ stage: "extract_audio" | "stt" | "translate" | "ready", message: string }`
- `pipeline-progress`: `{ current: number, total: number, percentage: number, message: string }`
- `pipeline-log`: `{ level: "info" | "warn" | "error", message: string }`
- `download-progress`: `{ model: string, downloaded: number, total: number, percentage: number }`

---

## 5. Model & Hardware Management

1. **GPU & Acceleration:**
   - Detects CUDA availability at startup. If CUDA is present and compiled with `--features cuda`, indicates "NVIDIA CUDA Acceleration" in UI.
   - If VRAM is exceeded during Whisper execution, graceful fallback to multi-threaded CPU executes automatically.
2. **Ollama Integration:**
   - On load, sends an HTTP request to `http://localhost:11434/api/tags` to fetch installed models.
   - If connection succeeds, populates the dropdown. If unreachable, displays helpful status badge and quick toggle to cloud API.
3. **Whisper Model Weights:**
   - Checks `~/.cache/auto-subtitle/` for `ggml-<model>.bin`.
   - Any missing model can be triggered for download via UI with progress bar without freezing the interface.

---

## 6. Subtitle Styling Presets

The UI provides 4 built-in ASS style presets rendered dynamically both in the video preview overlay and passed to `SubtitleFormatter` during export:

| Preset Name | Primary Color | Outline / Border | Font | Bilingual Position |
| :--- | :--- | :--- | :--- | :--- |
| **YouTube Yellow** | Yellow (`#FFDE00`) | Black 2px outline | Arial / Inter Bold | Translated (large yellow) over Original (small white) |
| **Classic White** | Pure White (`#FFFFFF`) | Black 2px outline | Arial / Roboto | Translated (white) over Original (subtle gray) |
| **Netflix Modern** | Off-White (`#F0F0F0`) | Translucent black box | Sans-serif Medium | Centered bottom with background strip |
| **Bilingual Highlight** | Cyan (`#00FFFF`) | Dark Blue 2px | Inter SemiBold | Top: Original / Bottom: Translated |

---

## 7. Error Handling & Edge Cases

1. **Path Safety:** The destination output path is validated to prevent overwriting the source file. Defaults to `<input_stem>_subtitled.<ext>`.
2. **Cancellation:** `CancellationToken` in `AppState` is triggered when the user clicks "Cancel". Child FFmpeg instances are killed and temporary `.wav` files deleted.
3. **Large Video Streaming:** Video playback utilizes Tauri's streaming file protocol to avoid high RAM consumption.
4. **Network Failures during Cloud STT / Translation:** Retries up to 3 times on transient network errors before reporting a clear error message in the log drawer.

---

## 8. Testing Strategy

1. **Unit Tests (Rust):**
   - Validate serialization and deserialization of `ExportRequest` and `SubtitleSegment`.
   - Test ASS/SRT formatting with all 4 style presets.
   - Test `CancellationToken` cancellation path.
2. **Component Tests (React):**
   - Test subtitle row editing (input change, split, merge, delete).
   - Test search and replace filter logic.
   - Test active time synchronization between video playback time and subtitle rows.
3. **Integration Test (End-to-End):**
   - Automated test using existing sample `jfk_video.mp4` and `jfk.flac`:
   - Run pipeline ➔ generate segments ➔ edit line ➔ export softsub/hardsub ➔ assert output file exists with non-zero size and valid subtitle stream.
