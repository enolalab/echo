# Echo Studio - Pipeline Progress Tracking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement real-time pipeline progress tracking with a 3px gradient progress bar on the top edge of the Bottom Drawer, detailed stage percentage (0% - 100%), segment counter (`X/Y đoạn`), and elapsed timer (`⏱ mm:ss`).

**Architecture:** Backend Rust emits `pipeline-progress` events via Tauri IPC (`app.emit("pipeline-progress", ...)`) using Whisper STT inference callback and batch translation callbacks. Frontend listens to `pipeline-progress` and animates a gradient progress line on `ProgressDrawer.tsx` while rendering formatted status counters.

**Tech Stack:** Rust (Tauri v2, whisper-rs, tokio), React 18, TypeScript, Tailwind CSS, Lucide icons, Vitest.

## Global Constraints
- Target 100% test passing across all existing tests (51 core tests + 5 Tauri backend tests + 8 UI tests).
- Never block the Tauri event loop or UI thread.
- Preserve full backward compatibility with the existing CLI command line tool.
- Provide graceful browser fallback for development and previewing in standard browsers.

---

### Task 1: Backend Event Types & Serialization

**Files:**
- Modify: `src-tauri/src/types.rs`
- Test: `src-tauri/tests/test_commands.rs`

**Interfaces:**
- Produces: `PipelineProgressEvent` struct with fields `stage: String`, `message: String`, `percentage: f64`, `current_step: Option<usize>`, `total_steps: Option<usize>`, `elapsed_secs: u64`.

- [ ] **Step 1: Add `PipelineProgressEvent` to `src-tauri/src/types.rs`**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PipelineProgressEvent {
    pub stage: String,
    pub message: String,
    pub percentage: f64,
    pub current_step: Option<usize>,
    pub total_steps: Option<usize>,
    pub elapsed_secs: u64,
}
```

- [ ] **Step 2: Add unit test in `src-tauri/tests/test_commands.rs`**
Verify serialization and deserialization of `PipelineProgressEvent`.

- [ ] **Step 3: Run `cargo test -p auto-subtitle-desktop`**
Verify the test passes.

- [ ] **Step 4: Commit changes**
`git commit -m "feat(tauri): add PipelineProgressEvent struct and serialization test"`

---

### Task 2: Core Pipeline Translation Callback Support

**Files:**
- Modify: `src/pipeline.rs`
- Test: `tests/pipeline_test.rs`

**Interfaces:**
- Produces: `translate_all_with_progress` (or updated `translate_all`) taking an optional progress callback `Option<&mut dyn FnMut(usize, usize)>`.

- [ ] **Step 1: Write unit test in `tests/pipeline_test.rs` verifying progress callback gets called after each batch**
- [ ] **Step 2: Update `src/pipeline.rs` to invoke the progress callback reporting `(translated_count, total_count)`**
- [ ] **Step 3: Verify all 51 core tests pass via `cargo test`**
- [ ] **Step 4: Commit changes**
`git commit -m "feat(pipeline): add progress callback hook to translation stage"`

---

### Task 3: Tauri Pipeline Progress Emission

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/tests/test_commands.rs`

**Interfaces:**
- Consumes: `translate_all_with_progress`, `whisper_rs::FullParams::set_progress_callback`
- Produces: Real-time `app.emit("pipeline-progress", PipelineProgressEvent)` during audio extraction (0-10%), Whisper STT (10-60%), translation (60-98%), and completion (100%).

- [ ] **Step 1: Implement progress emission in `src-tauri/src/commands.rs`**
  - Record `start_time = std::time::Instant::now()`.
  - Phase 1: Emit 0% -> 10% on audio extraction.
  - Phase 2: Attach Whisper STT callback mapping 0-100% to 10-60%.
  - Phase 3: Pass translation callback mapping translated segments to 60-98%.
  - Phase 4: Emit 100% when ready.
- [ ] **Step 2: Run `cargo test --manifest-path src-tauri/Cargo.toml`**
- [ ] **Step 3: Commit changes**
`git commit -m "feat(tauri): emit weighted pipeline-progress events across all stages"`

---

### Task 4: Frontend Types & Browser Simulation

**Files:**
- Modify: `ui/src/types/index.ts`
- Modify: `ui/src/lib/ipc.ts`
- Test: `ui/src/lib/ipc.test.ts`

**Interfaces:**
- Produces: `PipelineProgressEvent` TypeScript interface and progressive simulation in browser mode (`!isTauri()`).

- [ ] **Step 1: Add `PipelineProgressEvent` interface to `ui/src/types/index.ts`**
- [ ] **Step 2: Update `ui/src/lib/ipc.ts` to dispatch simulated progress events in browser mode**
- [ ] **Step 3: Run `npm --prefix ui run test` to verify Vitest**
- [ ] **Step 4: Commit changes**
`git commit -m "feat(ui): update types and mock IPC progress simulation"`

---

### Task 5: ProgressDrawer UI Component & App Integration

**Files:**
- Modify: `ui/src/components/ProgressDrawer.tsx`
- Modify: `ui/src/App.tsx`
- Create: `ui/src/components/ProgressDrawer.test.tsx`

**Interfaces:**
- Consumes: `PipelineProgressEvent`
- Produces: 3px animated top progress bar, center status display `[ 42% ] Đang dịch 18/45 đoạn • ⏱ 00:28s`.

- [ ] **Step 1: Create unit test `ui/src/components/ProgressDrawer.test.tsx`**
  - Test progress bar width matches `percentage%`.
  - Test percentage badge, segment counter, and elapsed time formatting.
- [ ] **Step 2: Implement progress bar and status formatting in `ui/src/components/ProgressDrawer.tsx`**
- [ ] **Step 3: Update `ui/src/App.tsx` to listen to `pipeline-progress` and pass to `ProgressDrawer`**
- [ ] **Step 4: Run frontend tests & build (`npm --prefix ui run test && npm --prefix ui run build`)**
- [ ] **Step 5: Commit changes**
`git commit -m "feat(ui): implement top gradient progress bar and formatted counters in ProgressDrawer"`

---

### Task 6: End-to-End Verification

- [ ] **Step 1: Run full test suite: `cargo test` and `npm --prefix ui run test`**
- [ ] **Step 2: Run `cargo build -p auto-subtitle-desktop`**
- [ ] **Step 3: Verify the application runs smoothly in `npm run dev`**
