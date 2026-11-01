# Echo Studio - Pipeline Progress Tracking Design Specification

- **Date:** 2026-11-01
- **Status:** Approved
- **Scope:** Desktop UI (`src-tauri` and `ui/`) + Core library progress emission hooks
- **Related Spec:** [docs/superpowers/specs/2026-11-01-desktop-ui-design.md](file:///home/k0walski/Code/auto-subtitle/docs/superpowers/specs/2026-11-01-desktop-ui-design.md)

---

## 1. Overview & Objective

When running transcription and translation on video/audio files, the process can take from dozens of seconds to multiple minutes depending on the audio duration, model size (e.g. Whisper base vs medium), and translation engine (Ollama local vs Cloud API).

Currently, the UI displays only static stage breadcrumb chips without dynamic percentage, elapsed duration, or item counters, leaving users unsure of execution progress.

This specification defines a real-time, weighted **Progress Tracking System** featuring:
1. A slim 3px gradient progress bar seamlessly embedded along the top edge of the Bottom Drawer.
2. Accurate overall percentage calculation (0% - 100%) weighted across stages.
3. Informative center status readout with percentage badge (`[ 42% ]`), step counter (`Đang dịch 18/45 đoạn`), and formatted elapsed timer (`⏱ 00:28s`).
4. Real-time progress emission from Whisper STT inference callback and batch-by-batch LLM translation callback.

---

## 2. Architecture & Data Flow

### 2.1 IPC Event Definition: `pipeline-progress`

Backend Rust emits `pipeline-progress` events containing:

```typescript
export interface PipelineProgressEvent {
  stage: 'extract_audio' | 'stt' | 'translate' | 'ready';
  message: string;
  percentage: number;       // Floating point number 0.0 - 100.0
  current_step?: number;     // e.g. segment 15
  total_steps?: number;      // e.g. total 45 segments
  elapsed_secs: number;      // Seconds elapsed since pipeline start
}
```

Rust counterpart in `src-tauri/src/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineProgressEvent {
    pub stage: String,
    pub message: String,
    pub percentage: f64,
    pub current_step: Option<usize>,
    pub total_steps: Option<usize>,
    pub elapsed_secs: u64,
}
```

### 2.2 Stage Weight Distribution (0% - 100%)

The total execution time is distributed across 4 pipeline phases:

| Phase | Stage Key | Weight (%) | Description |
|---|---|---|---|
| 1. Extract Audio | `extract_audio` | 0% $\rightarrow$ 10% | FFmpeg 16kHz mono WAV extraction |
| 2. Whisper STT | `stt` | 10% $\rightarrow$ 60% | Model check/download + Whisper inference with callback (0% - 100% of Whisper maps to 10% - 60% overall) |
| 3. LLM Translation | `translate` | 60% $\rightarrow$ 98% | Batch-by-batch translation; `60.0 + (done / total) * 38.0` |
| 4. Ready | `ready` | 100% | Segments sanitized and delivered to Subtitle Editor |

---

## 3. Backend Implementation (Rust)

### 3.1 Whisper STT Inference Callback
- In `src-tauri/src/commands.rs`, before calling inference, configure `whisper_rs::FullParams::set_progress_callback`.
- The callback reports Whisper's internal chunk processing progress (0 to 100).
- Emits `pipeline-progress` with `percentage = 10.0 + (progress as f64 * 0.50)` and updated `elapsed_secs`.

### 3.2 LLM Translation Progress Emission
- In `auto_subtitle::pipeline::translate_all` (or in the Tauri command pipeline wrapper), after translating each batch of subtitle segments:
  - Calculate `translated_count` vs `total_segments`.
  - Calculate `percentage = 60.0 + ((translated_count as f64 / total_segments as f64) * 38.0)`.
  - Emit `pipeline-progress` with `current_step = Some(translated_count)` and `total_steps = Some(total_segments)`.

### 3.3 Cancellation & Error State
- On cancellation (`cancel_pipeline`):
  - Emit final event or reset `isRunning = false` and reset progress bar to 0%.
- On error:
  - Emit error message into `pipeline-log` and reset progress bar to error state (turns red, stops timer).

---

## 4. Frontend UI Implementation (React + Tailwind)

### 4.1 Top-Border Progress Bar (`ProgressDrawer.tsx`)
- Container is positioned at the top of the Bottom Drawer:
  - Height: `h-[3px]`.
  - Width: `style={{ width: `${Math.min(100, Math.max(0, percentage))}%` }}`.
  - Transition: `transition-all duration-300 ease-out`.
  - Background styling:
    - Running: `bg-gradient-to-r from-blue-500 via-indigo-500 to-emerald-400`.
    - Ready (100%): `bg-emerald-500`.
    - Error: `bg-rose-500`.

### 4.2 Center Readout
Format:
```
[ 42% ]  Đang dịch 18/45 đoạn phụ đề  •  ⏱ 00:28s
```
- Percentage Badge: monospace font `font-mono font-bold text-blue-400 bg-blue-950/60 px-2 py-0.5 rounded border border-blue-800/40`.
- Detail text: dynamic stage message or segment counter.
- Timer: formatted `mm:ss` from `elapsed_secs`.

### 4.3 Web Fallback Simulator
- In `ui/src/lib/ipc.ts`, when running in browser preview mode (`!isTauri()`):
  - Simulate progressive stage events every 250ms (0% $\rightarrow$ 10% $\rightarrow$ 35% $\rightarrow$ 60% $\rightarrow$ 85% $\rightarrow$ 100%) so UI developers can preview progress animations without needing native backend.

---

## 5. Verification & Testing Plan

1. **Unit Tests (Backend):**
   - Test `PipelineProgressEvent` serialization and deserialization.
   - Verify progress formula clamps between 0.0 and 100.0.
2. **Frontend Component Tests (Vitest):**
   - Test `ProgressDrawer` renders progress bar with correct width style attribute.
   - Test timer formatting (`0s` $\rightarrow$ `00:00`, `65s` $\rightarrow$ `01:05`).
   - Test counter rendering (`current_step` / `total_steps`).
3. **End-to-End Verification:**
   - Run desktop app and trigger a video transcription.
   - Verify progress bar moves smoothly from 0% through audio extraction, STT, translation, to 100%.
