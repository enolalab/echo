# Desktop UI (Tauri + React) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a cross-platform desktop application for Echo (`auto-subtitle`) using Tauri v2, React, TypeScript, and Tailwind CSS, featuring an all-in-one studio layout for one-click subtitle generation, bidirectional video preview & editing, and flexible exporting.

**Architecture:** Refactor the existing Rust crate into a shared library (`auto-subtitle-core`) and a CLI binary wrapper. Build a Tauri v2 backend in `src-tauri/` that invokes core pipeline and formatting operations via async Tokio tasks and streams real-time events. Construct a Vite-powered React frontend in `ui/` with a video player, bidirectional subtitle editor, model detection, and export controls.

**Tech Stack:** Rust 2021, Tauri v2, Tokio, React 18, TypeScript, Tailwind CSS, Vite, `@tanstack/react-virtual`, Lucide React.

## Global Constraints

- Never break or modify the existing CLI interface or flags (`auto-subtitle -i <video> ...`).
- Never overwrite source video files; default outputs must append `_subtitled`.
- Video streaming must use Tauri's asset protocol (`convertFileSrc`) to avoid high RAM usage.
- All tasks must be fully typed and adhere to TDD with automated verification.

---

### Task 1: Core Library Refactor & Workspace Setup

**Files:**
- Modify: `Cargo.toml`
- Create: `src/lib.rs`
- Modify: `src/main.rs`
- Test: `tests/test_cli_unchanged.rs`

**Interfaces:**
- Consumes: Existing `pipeline.rs`, `media.rs`, `downloader.rs`, `stt/`, `translate/`, `subtitle/`.
- Produces: `auto_subtitle::pipeline::Pipeline`, `auto_subtitle::subtitle::SubtitleSegment`, `auto_subtitle::downloader::*`, `auto_subtitle::config::Config`.

- [ ] **Step 1: Write integration test verifying CLI binary and library exports**

```rust
// tests/test_core_exports.rs
use auto_subtitle::subtitle::SubtitleSegment;

#[test]
fn test_subtitle_segment_types() {
    let seg = SubtitleSegment {
        id: 1,
        start_ms: 1000,
        end_ms: 4000,
        text: "Hello".to_string(),
        original_text: Some("Hello".to_string()),
        translated_text: Some("Xin chào".to_string()),
    };
    assert_eq!(seg.id, 1);
    assert_eq!(seg.start_ms, 1000);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test test_core_exports`  
Expected: FAIL (cannot find `auto_subtitle` crate/module)

- [ ] **Step 3: Expose modules in `src/lib.rs` and update `Cargo.toml`**

Update `Cargo.toml` to define both `[lib]` and `[[bin]]`:
```toml
[lib]
name = "auto_subtitle"
path = "src/lib.rs"

[[bin]]
name = "auto-subtitle"
path = "src/main.rs"
```

In `src/lib.rs`:
```rust
pub mod cli;
pub mod config;
pub mod downloader;
pub mod media;
pub mod pipeline;
pub mod stt;
pub mod subtitle;
pub mod translate;
```

Update `src/main.rs` to import from `auto_subtitle`.

- [ ] **Step 4: Run tests and CLI check**

Run: `cargo test`  
Expected: PASS all existing tests and `test_core_exports`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/lib.rs src/main.rs tests/test_core_exports.rs
git commit -m "refactor: export core modules as library and keep CLI binary intact"
```

---

### Task 2: Tauri v2 Project Setup & IPC Type Contracts

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/types.rs`
- Create: `src-tauri/src/main.rs`
- Test: `src-tauri/src/types.rs` (unit tests for Serde)

**Interfaces:**
- Consumes: `auto_subtitle::subtitle::SubtitleSegment`.
- Produces: `SystemInfo`, `PipelineConfig`, `ExportRequest`, `ProgressEvent`.

- [ ] **Step 1: Write Serde serialization unit tests for IPC data structures**

```rust
// In src-tauri/src/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_request_serialization() {
        let json = r#"{
            "video_path": "/path/video.mp4",
            "output_path": "/path/video_subtitled.mp4",
            "mode": "hardsub",
            "bilingual": true,
            "style_preset": "youtube_yellow",
            "segments": []
        }"#;
        let req: ExportRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.mode, ExportMode::Hardsub);
        assert_eq!(req.style_preset, StylePreset::YouTubeYellow);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`  
Expected: FAIL (file or types not yet defined)

- [ ] **Step 3: Scaffold `src-tauri` and implement `types.rs`**

Create `src-tauri/Cargo.toml` referencing Tauri v2 and `auto_subtitle = { path = ".." }`.  
Implement `types.rs` with `Serialize, Deserialize, Clone, Debug` for `SystemInfo`, `WhisperModelInfo`, `PipelineConfig`, `ExportRequest`, `ExportMode`, `StylePreset`.

- [ ] **Step 4: Configure `tauri.conf.json` with permissions & asset protocol**

Configure window title "Echo Studio", size 1280x800, min size 960x600, custom protocol for local video asset streaming (`asset: true`), and file dialog permissions.

- [ ] **Step 5: Run tests to verify passing**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`  
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/
git commit -m "feat: scaffold Tauri v2 backend and define IPC type contracts"
```

---

### Task 3: Tauri IPC Backend Commands & Event Streaming

**Files:**
- Create: `src-tauri/src/state.rs`
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src-tauri/tests/test_commands.rs`

**Interfaces:**
- Consumes: `auto_subtitle::pipeline`, `auto_subtitle::downloader`, `auto_subtitle::media`.
- Produces: `get_system_info`, `download_whisper_model`, `start_pipeline`, `cancel_pipeline`, `export_subtitles`, `open_in_file_manager`.

- [ ] **Step 1: Write integration tests for commands logic**

```rust
// src-tauri/tests/test_commands.rs
#[tokio::test]
async fn test_system_info_detection() {
    let info = auto_subtitle_desktop::commands::detect_system_info().await;
    assert!(info.whisper_models.len() >= 4);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test test_commands`  
Expected: FAIL (`commands::detect_system_info` not found)

- [ ] **Step 3: Implement `state.rs` and `commands.rs`**

- `state.rs`: Store `Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>` for task cancellation.
- `commands::get_system_info`: Inspect `which nvcc` / CUDA support, check `~/.cache/auto-subtitle/` for GGML files, probe `http://localhost:11434/api/tags` with 1-second timeout.
- `commands::download_whisper_model`: Download GGML model via `downloader.rs` and emit `download-progress` events.
- `commands::start_pipeline`: Launch background task, extract audio, run STT and translation, emitting `pipeline-stage`, `pipeline-progress`, `pipeline-log`. Return `Vec<SubtitleSegment>`.
- `commands::cancel_pipeline`: Fire cancellation channel and abort background process.
- `commands::export_subtitles`: Generate ASS/SRT subtitles from edited segments with chosen style preset, invoke `media::burn_subtitles` or `media::mux_subtitles`.
- `commands::open_in_file_manager`: Call OS native file explorer.

- [ ] **Step 4: Run tests to verify passing**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/state.rs src-tauri/src/commands.rs src-tauri/src/main.rs src-tauri/tests/
git commit -m "feat(tauri): implement backend IPC commands and event streaming"
```

---

### Task 4: Frontend Scaffolding, Theme & IPC Client

**Files:**
- Create: `ui/package.json`
- Create: `ui/vite.config.ts`
- Create: `ui/tailwind.config.js`
- Create: `ui/src/types/index.ts`
- Create: `ui/src/lib/ipc.ts`
- Test: `ui/src/lib/ipc.test.ts`

**Interfaces:**
- Consumes: `@tauri-apps/api`.
- Produces: Typed TypeScript wrappers `api.getSystemInfo()`, `api.startPipeline()`, `api.exportSubtitles()`, `useTauriEvent()`.

- [ ] **Step 1: Write mock test for typed IPC client**

```typescript
// ui/src/lib/ipc.test.ts
import { describe, it, expect, vi } from 'vitest';
import { getSystemInfo } from './ipc';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({
    has_cuda: false,
    ollama_running: true,
    ollama_models: ['qwen2.5:7b'],
    whisper_models: []
  }),
}));

describe('IPC Client', () => {
  it('fetches system info correctly', async () => {
    const info = await getSystemInfo();
    expect(info.ollama_running).toBe(true);
    expect(info.ollama_models).toContain('qwen2.5:7b');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test`  
Expected: FAIL (modules not found)

- [ ] **Step 3: Setup Vite React TypeScript app in `ui/`**

Initialize `ui/package.json` with React 18, `@tauri-apps/api@^2.0.0`, Tailwind CSS, Lucide React, `@tanstack/react-virtual`.
Implement `ui/src/types/index.ts` and `ui/src/lib/ipc.ts`.

- [ ] **Step 4: Run tests to verify passing**

Run: `npm --prefix ui test`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui/
git commit -m "feat(ui): scaffold React + Tailwind app and typed Tauri IPC client"
```

---

### Task 5: HeaderBar & Model Management Component

**Files:**
- Create: `ui/src/components/HeaderBar.tsx`
- Create: `ui/src/components/ModelSelectorModal.tsx`
- Create: `ui/src/hooks/useSystemInfo.ts`
- Test: `ui/src/components/HeaderBar.test.tsx`

**Interfaces:**
- Consumes: `api.getSystemInfo()`, `api.downloadWhisperModel()`.
- Produces: Header state updates (`videoPath`, `whisperModel`, `targetLang`, `stylePreset`, `isBilingual`).

- [ ] **Step 1: Write test for HeaderBar interactions**

```tsx
// ui/src/components/HeaderBar.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { HeaderBar } from './HeaderBar';

test('renders Start button disabled when no video selected', () => {
  render(<HeaderBar videoPath="" onStart={vi.fn()} isRunning={false} />);
  expect(screen.getByRole('button', { name: /bắt đầu/i })).toBeDisabled();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test HeaderBar`  
Expected: FAIL

- [ ] **Step 3: Implement `HeaderBar` and model selection UI**

Include drag-drop file zone / open dialog button, Whisper model dropdown with download triggers, Ollama model dropdown, Style preset select, Bilingual toggle switch, and "Start Auto-Sub" button.

- [ ] **Step 4: Run test to verify passing**

Run: `npm --prefix ui test HeaderBar`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/HeaderBar* ui/src/hooks/useSystemInfo.ts
git commit -m "feat(ui): implement HeaderBar with video picker and model detection"
```

---

### Task 6: VideoPlayer & Realtime Subtitle Overlay

**Files:**
- Create: `ui/src/components/VideoPlayer.tsx`
- Create: `ui/src/components/SubtitleOverlay.tsx`
- Create: `ui/src/lib/presets.ts`
- Test: `ui/src/components/SubtitleOverlay.test.tsx`

**Interfaces:**
- Consumes: `videoPath`, `currentTime`, `segments: SubtitleSegment[]`, `preset: StylePreset`, `isBilingual`.
- Produces: Player seek callbacks, playback play/pause state.

- [ ] **Step 1: Write unit test for SubtitleOverlay rendering**

```tsx
// ui/src/components/SubtitleOverlay.test.tsx
import { render, screen } from '@testing-library/react';
import { SubtitleOverlay } from './SubtitleOverlay';

test('displays active subtitle text at matching timestamp', () => {
  const segments = [{
    id: 1,
    start_ms: 1000,
    end_ms: 4000,
    original_text: "Hello",
    translated_text: "Xin chào",
  }];
  render(
    <SubtitleOverlay
      currentTimeMs={2500}
      segments={segments}
      preset="youtube_yellow"
      isBilingual={true}
    />
  );
  expect(screen.getByText("Xin chào")).toBeInTheDocument();
  expect(screen.getByText("Hello")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test SubtitleOverlay`  
Expected: FAIL

- [ ] **Step 3: Implement `VideoPlayer` and `SubtitleOverlay`**

- `VideoPlayer`: HTML5 video tag using `convertFileSrc(videoPath)`. Custom control bar: Play/Pause, Seek slider, timecode text (`00:01:23 / 00:04:12`), speed controls, loop segment key (`R`).
- `SubtitleOverlay`: Floating subtitle display with CSS classes representing the 4 presets (*YouTube Yellow*, *Classic White*, *Netflix Modern*, *Bilingual Highlight*).

- [ ] **Step 4: Run test to verify passing**

Run: `npm --prefix ui test SubtitleOverlay`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/VideoPlayer.tsx ui/src/components/SubtitleOverlay* ui/src/lib/presets.ts
git commit -m "feat(ui): implement VideoPlayer with real-time styled subtitle overlay"
```

---

### Task 7: SubtitleEditor Table with Bidirectional Sync

**Files:**
- Create: `ui/src/components/SubtitleEditor.tsx`
- Create: `ui/src/components/SubtitleRow.tsx`
- Create: `ui/src/hooks/useSubtitleSync.ts`
- Test: `ui/src/components/SubtitleEditor.test.tsx`

**Interfaces:**
- Consumes: `segments: SubtitleSegment[]`, `currentTimeMs: number`, `onSeek: (ms: number) => void`.
- Produces: `onUpdateSegment: (seg: SubtitleSegment) => void`, `onSplitSegment`, `onMergeSegment`, `onDeleteSegment`.

- [ ] **Step 1: Write test for inline editing and seek trigger**

```tsx
// ui/src/components/SubtitleEditor.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { SubtitleEditor } from './SubtitleEditor';

test('clicking a row calls onSeek with segment start_ms', () => {
  const onSeek = vi.fn();
  const segments = [{
    id: 1,
    start_ms: 1500,
    end_ms: 3500,
    original_text: "Hello",
    translated_text: "Xin chào",
  }];
  render(
    <SubtitleEditor
      segments={segments}
      currentTimeMs={0}
      onSeek={onSeek}
      onUpdate={vi.fn()}
    />
  );
  fireEvent.click(screen.getByText("Xin chào"));
  expect(onSeek).toHaveBeenCalledWith(1500);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test SubtitleEditor`  
Expected: FAIL

- [ ] **Step 3: Implement `SubtitleEditor` with virtualization & editing**

- Use `@tanstack/react-virtual` for smooth 60fps scrolling with large subtitle sets.
- Bidirectional sync: Highlight row when `currentTimeMs >= seg.start_ms && currentTimeMs <= seg.end_ms`. Auto-scroll active row into view.
- Row actions: Split, Merge, Delete, Time nudge (`±0.1s`).
- Global search & replace input filter.

- [ ] **Step 4: Run test to verify passing**

Run: `npm --prefix ui test SubtitleEditor`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/SubtitleEditor* ui/src/components/SubtitleRow.tsx ui/src/hooks/useSubtitleSync.ts
git commit -m "feat(ui): implement SubtitleEditor with bidirectional sync and inline editing"
```

---

### Task 8: ProgressDrawer & ExportModal Integration

**Files:**
- Create: `ui/src/components/ProgressDrawer.tsx`
- Create: `ui/src/components/ExportModal.tsx`
- Modify: `ui/src/App.tsx`
- Test: `ui/src/components/ExportModal.test.tsx`

**Interfaces:**
- Consumes: Pipeline progress events, edited `SubtitleSegment[]`, selected preset.
- Produces: Calls `api.exportSubtitles()` and `api.openInFileManager()`.

- [ ] **Step 1: Write test for ExportModal submitting payload**

```tsx
// ui/src/components/ExportModal.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { ExportModal } from './ExportModal';

test('triggers export callback with chosen mode and style preset', () => {
  const onExport = vi.fn();
  render(
    <ExportModal
      isOpen={true}
      onClose={vi.fn()}
      onExport={onExport}
      defaultOutputPath="/path/output.mp4"
    />
  );
  fireEvent.click(screen.getByRole('button', { name: /xuất video/i }));
  expect(onExport).toHaveBeenCalledWith(expect.objectContaining({
    mode: 'hardsub',
  }));
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test ExportModal`  
Expected: FAIL

- [ ] **Step 3: Implement `ProgressDrawer`, `ExportModal`, and wire `App.tsx`**

- `ProgressDrawer`: Stage indicator chips (`Audio`, `STT`, `Translate`), animated percentage bar, log output terminal, Cancel button.
- `ExportModal`: Radio selector for Hardsub / Softsub / Subs-only, file destination picker, Render button with completion notification.
- In `App.tsx`: Combine HeaderBar, VideoPlayer, SubtitleEditor, ProgressDrawer, and ExportModal into the Studio All-in-One layout.

- [ ] **Step 4: Run test to verify passing**

Run: `npm --prefix ui test ExportModal`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/ProgressDrawer.tsx ui/src/components/ExportModal* ui/src/App.tsx
git commit -m "feat(ui): integrate ProgressDrawer, ExportModal, and App Studio layout"
```

---

### Task 9: End-to-End Verification & Documentation

**Files:**
- Modify: `README.md`
- Test: `tests/test_e2e_desktop.rs`

- [ ] **Step 1: Write integration test verifying CLI and export operations with fixtures**

Use `jfk_video.mp4` to run an export pass with `ExportRequest` using `SubtitleFormatter` and `media::burn_subtitles`. Verify resulting file exists and contains video stream.

- [ ] **Step 2: Run integration tests**

Run: `cargo test`  
Expected: PASS all unit and integration tests.

- [ ] **Step 3: Verify build for frontend and Tauri**

Run:
```bash
npm --prefix ui run build
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: PASS with 0 build errors.

- [ ] **Step 4: Update `README.md` with Desktop UI instructions**

Document how to launch desktop mode (`cargo tauri dev`) or build installer (`cargo tauri build`), alongside existing CLI documentation.

- [ ] **Step 5: Commit**

```bash
git add README.md tests/
git commit -m "docs: add desktop app usage and verify end-to-end build"
```
