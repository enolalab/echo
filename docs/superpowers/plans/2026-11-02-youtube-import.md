# YouTube Video & Playlist Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to import single YouTube videos or full/partial playlists directly into `auto-subtitle` via a managed `yt-dlp` binary, downloading them as MP4 files and automatically queueing them into the subtitle pipeline.

**Architecture:** A Rust backend module `src/youtube` manages `yt-dlp` binary location/auto-download, fast JSON metadata inspection, and streaming subprocess downloads. Tauri commands expose inspection and download operations with progress events. A React modal `YoutubeImportModal` handles URL input, previewing/selecting playlist items, real-time download progress, and passing completed files into the `JobList`.

**Tech Stack:** Rust (tokio, serde, reqwest, regex, which), Tauri v2 IPC & Events, React 18, TypeScript, Tailwind CSS, Lucide icons, Vitest.

## Global Constraints

- Downloads must be packaged into standard H.264/AAC MP4 compatible with HTML5 `<video>`.
- `yt-dlp` must be searched first in system `PATH`, then app data directory `auto-subtitle/bin/yt-dlp`. If missing, auto-download from official GitHub releases.
- Playlist inspection must use `--dump-single-json --flat-playlist` for fast metadata extraction without downloading media content.
- Video downloads must stream progress via `youtube-download-progress` Tauri event.
- Completed downloads must automatically call `addJobs` into `JobList` in `queued` state.

---

### Task 1: YouTube Data Models & URL Parser (Rust)

**Files:**
- Create: `src/youtube/mod.rs`
- Create: `src/youtube/model.rs`
- Create: `src/youtube/url.rs`
- Modify: `src/lib.rs`
- Test: `tests/youtube_url_test.rs`

**Interfaces:**
- Consumes: `serde::{Serialize, Deserialize}`, `regex::Regex`
- Produces:
  - `pub enum YoutubeUrlType { SingleVideo(String), Playlist(String), VideoWithPlaylist { video_id: String, playlist_id: String } }`
  - `pub fn parse_youtube_url(url: &str) -> Result<YoutubeUrlType, String>`
  - Structs: `YtDlpStatus`, `YoutubeVideoItem`, `YoutubeInspectResult`, `YoutubeDownloadRequest`, `YoutubeProgressPayload`

- [ ] **Step 1: Write the failing tests for YouTube URL parsing**

```rust
// tests/youtube_url_test.rs
use auto_subtitle::youtube::url::{parse_youtube_url, YoutubeUrlType};

#[test]
fn test_parse_single_video_standard() {
    let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    let res = parse_youtube_url(url).expect("should parse");
    assert_eq!(res, YoutubeUrlType::SingleVideo("dQw4w9WgXcQ".to_string()));
}

#[test]
fn test_parse_single_video_shortlink() {
    let url = "https://youtu.be/dQw4w9WgXcQ?t=42";
    let res = parse_youtube_url(url).expect("should parse");
    assert_eq!(res, YoutubeUrlType::SingleVideo("dQw4w9WgXcQ".to_string()));
}

#[test]
fn test_parse_shorts() {
    let url = "https://www.youtube.com/shorts/dQw4w9WgXcQ";
    let res = parse_youtube_url(url).expect("should parse");
    assert_eq!(res, YoutubeUrlType::SingleVideo("dQw4w9WgXcQ".to_string()));
}

#[test]
fn test_parse_pure_playlist() {
    let url = "https://www.youtube.com/playlist?list=PLrAXtmErZgOdP_8GztsuKi9nh508_u4tM";
    let res = parse_youtube_url(url).expect("should parse");
    assert_eq!(res, YoutubeUrlType::Playlist("PLrAXtmErZgOdP_8GztsuKi9nh508_u4tM".to_string()));
}

#[test]
fn test_parse_video_with_playlist() {
    let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLrAXtmErZgOdP_8GztsuKi9nh508_u4tM";
    let res = parse_youtube_url(url).expect("should parse");
    assert_eq!(res, YoutubeUrlType::VideoWithPlaylist {
        video_id: "dQw4w9WgXcQ".to_string(),
        playlist_id: "PLrAXtmErZgOdP_8GztsuKi9nh508_u4tM".to_string(),
    });
}

#[test]
fn test_parse_invalid_url() {
    let url = "https://example.com/not-youtube";
    assert!(parse_youtube_url(url).is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test youtube_url_test`
Expected: FAIL (module or function not found)

- [ ] **Step 3: Implement data models and URL parser**

Create `src/youtube/model.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtDlpStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeVideoItem {
    pub id: String,
    pub title: String,
    pub duration_seconds: Option<f64>,
    pub thumbnail_url: Option<String>,
    pub channel_name: Option<String>,
    pub webpage_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeInspectResult {
    pub is_playlist: bool,
    pub playlist_id: Option<String>,
    pub title: String,
    pub items: Vec<YoutubeVideoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeDownloadRequest {
    pub items: Vec<YoutubeVideoItem>,
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeProgressPayload {
    pub video_id: String,
    pub index: usize,
    pub total_items: usize,
    pub stage: String,
    pub percentage: f32,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub saved_path: Option<String>,
    pub error: Option<String>,
}
```

Create `src/youtube/url.rs`:
```rust
use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YoutubeUrlType {
    SingleVideo(String),
    Playlist(String),
    VideoWithPlaylist { video_id: String, playlist_id: String },
}

pub fn parse_youtube_url(url: &str) -> Result<YoutubeUrlType, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("URL không được để trống".into());
    }

    let re_v = Regex::new(r#"(?:v=|\/v\/|\/embed\/|\/shorts\/|^youtu\.be\/|(?:\?|&)v=)([a-zA-Z0-9_-]{11})"#)
        .map_err(|e| e.to_string())?;
    let re_list = Regex::new(r#"(?:list=)([a-zA-Z0-9_-]+)"#)
        .map_err(|e| e.to_string())?;

    let video_id = re_v.captures(trimmed).map(|c| c[1].to_string());
    let playlist_id = re_list.captures(trimmed).map(|c| c[1].to_string());

    match (video_id, playlist_id) {
        (Some(v), Some(p)) => Ok(YoutubeUrlType::VideoWithPlaylist { video_id: v, playlist_id: p }),
        (Some(v), None) => Ok(YoutubeUrlType::SingleVideo(v)),
        (None, Some(p)) => Ok(YoutubeUrlType::Playlist(p)),
        (None, None) => Err("Không nhận diện được link YouTube hợp lệ (video hoặc playlist)".into()),
    }
}
```

Create `src/youtube/mod.rs` and update `src/lib.rs`:
```rust
// src/youtube/mod.rs
pub mod model;
pub mod url;
pub mod manager;
```

In `src/lib.rs`:
```rust
pub mod youtube;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test youtube_url_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/youtube/ tests/youtube_url_test.rs src/lib.rs
git commit -m "feat(youtube): add YouTube data models and URL parser"
```

---

### Task 2: YtDlpManager — Binary Resolution, Inspection & Download Runner (Rust)

**Files:**
- Create: `src/youtube/manager.rs`
- Test: `tests/youtube_manager_test.rs`

**Interfaces:**
- Consumes: `YtDlpStatus`, `YoutubeInspectResult`, `YoutubeVideoItem`, `tokio::process::Command`
- Produces:
  - `pub struct YtDlpManager`
  - `pub async fn check_status(&self) -> YtDlpStatus`
  - `pub async fn ensure_binary(&self) -> anyhow::Result<PathBuf>`
  - `pub async fn inspect_url(&self, url: &str) -> anyhow::Result<YoutubeInspectResult>`
  - `pub async fn download_item<F>(&self, item: &YoutubeVideoItem, output_dir: &Path, on_progress: F) -> anyhow::Result<PathBuf> where F: Fn(f32, Option<String>, Option<String>) + Send + Sync + 'static`

- [ ] **Step 1: Write test for inspect parser logic**

```rust
// tests/youtube_manager_test.rs
use auto_subtitle::youtube::manager::parse_inspect_json;

#[test]
fn test_parse_single_video_json() {
    let json_data = r#"{
        "_type": "video",
        "id": "dQw4w9WgXcQ",
        "title": "Rick Astley - Never Gonna Give You Up",
        "duration": 213,
        "thumbnail": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
        "uploader": "RickAstleyVEVO",
        "webpage_url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    }"#;

    let res = parse_inspect_json(json_data).expect("parse json");
    assert!(!res.is_playlist);
    assert_eq!(res.title, "Rick Astley - Never Gonna Give You Up");
    assert_eq!(res.items.len(), 1);
    assert_eq!(res.items[0].id, "dQw4w9WgXcQ");
    assert_eq!(res.items[0].duration_seconds, Some(213.0));
}

#[test]
fn test_parse_playlist_json() {
    let json_data = r#"{
        "_type": "playlist",
        "id": "PLtest123",
        "title": "Sample Playlist",
        "entries": [
            {
                "id": "v1",
                "title": "Video 1",
                "duration": 60,
                "thumbnails": [{"url": "https://example.com/1.jpg"}],
                "uploader": "Uploader 1",
                "url": "https://www.youtube.com/watch?v=v1"
            },
            {
                "id": "v2",
                "title": "Video 2",
                "duration": 120,
                "thumbnails": [{"url": "https://example.com/2.jpg"}],
                "uploader": "Uploader 2",
                "url": "https://www.youtube.com/watch?v=v2"
            }
        ]
    }"#;

    let res = parse_inspect_json(json_data).expect("parse playlist");
    assert!(res.is_playlist);
    assert_eq!(res.title, "Sample Playlist");
    assert_eq!(res.items.len(), 2);
    assert_eq!(res.items[1].id, "v2");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test youtube_manager_test`
Expected: FAIL (function `parse_inspect_json` not found)

- [ ] **Step 3: Implement `YtDlpManager` and `parse_inspect_json`**

In `src/youtube/manager.rs`:
- Implement binary path discovery:
  1. Check `which::which("yt-dlp")`.
  2. Check `dirs::data_local_dir()/auto-subtitle/bin/yt-dlp` (or `.exe` on Windows).
- Implement `ensure_binary()`:
  - If missing, download platform-specific binary from GitHub releases with `reqwest`.
  - Set permissions `0o755` on Unix.
- Implement `parse_inspect_json(json_str: &str) -> anyhow::Result<YoutubeInspectResult>`:
  - Deserialize using `serde_json::Value`.
  - Parse both `_type == "playlist"` and single video objects.
- Implement `inspect_url(&self, url: &str)`:
  - Runs `yt-dlp --dump-single-json --flat-playlist --no-warnings <url>`.
- Implement `download_item()`:
  - Execute `yt-dlp` with args:
    `-f "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/b" --merge-output-format mp4 -o <output_template> --newline --progress-template "%(progress.downloaded_bytes)d %(progress.total_bytes)d %(progress.speed)f %(progress.eta)d" <url>`.
  - Parse progress output and invoke callback.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test youtube_manager_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/youtube/manager.rs tests/youtube_manager_test.rs
git commit -m "feat(youtube): implement YtDlpManager with inspect parser and download runner"
```

---

### Task 3: Tauri IPC Commands & Event Streaming (`src-tauri`)

**Files:**
- Create: `src-tauri/src/youtube_commands.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/types.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/tests/test_youtube_commands.rs`

**Interfaces:**
- Consumes: `auto_subtitle::youtube::manager::YtDlpManager`, Tauri `AppHandle`, `Emitter`
- Produces Tauri commands:
  - `check_yt_dlp_status`
  - `ensure_yt_dlp`
  - `inspect_youtube_url(url: String)`
  - `download_youtube_items(req: YoutubeDownloadRequest)`
  - `cancel_youtube_download()`

- [ ] **Step 1: Write integration test for youtube commands**

```rust
// src-tauri/tests/test_youtube_commands.rs
use auto_subtitle::youtube::model::{YoutubeVideoItem, YoutubeDownloadRequest};

#[test]
fn test_download_request_serialization() {
    let req = YoutubeDownloadRequest {
        items: vec![
            YoutubeVideoItem {
                id: "test1".into(),
                title: "Test Video".into(),
                duration_seconds: Some(120.0),
                thumbnail_url: None,
                channel_name: Some("Test Channel".into()),
                webpage_url: "https://youtube.com/watch?v=test1".into(),
            }
        ],
        output_dir: Some("/tmp/downloads".into()),
    };

    let serialized = serde_json::to_string(&req).expect("serialize");
    let deserialized: YoutubeDownloadRequest = serde_json::from_str(&serialized).expect("deserialize");
    assert_eq!(deserialized.items.len(), 1);
    assert_eq!(deserialized.items[0].id, "test1");
}
```

- [ ] **Step 2: Run test to verify dependencies build**

Run: `cargo test --test test_youtube_commands -p src-tauri`
Expected: PASS

- [ ] **Step 3: Implement `youtube_commands.rs` and register in `main.rs`**

Create `src-tauri/src/youtube_commands.rs`:
- Define `check_yt_dlp_status`, `ensure_yt_dlp`, `inspect_youtube_url`, `download_youtube_items`, `cancel_youtube_download`.
- Manage cancellation token via `AtomicBool` or `tokio::sync::watch` in Tauri state.
- In `download_youtube_items`, sequentially iterate items:
  - Emit `youtube-download-progress` event to frontend on progress change and completion.
- In `src-tauri/src/main.rs`:
  - Add `mod youtube_commands;`
  - Register new commands in `.invoke_handler(tauri::generate_handler![..., check_yt_dlp_status, ensure_yt_dlp, inspect_youtube_url, download_youtube_items, cancel_youtube_download])`.

- [ ] **Step 4: Verify cargo check on `src-tauri`**

Run: `cargo check -p src-tauri`
Expected: Success with 0 errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/youtube_commands.rs src-tauri/src/main.rs src-tauri/tests/test_youtube_commands.rs
git commit -m "feat(tauri): add YouTube IPC commands and download progress event emitter"
```

---

### Task 4: Frontend Types & IPC Service Layer (TypeScript)

**Files:**
- Create: `ui/src/types/youtube.ts`
- Modify: `ui/src/types/index.ts`
- Modify: `ui/src/lib/ipc.ts`
- Test: `ui/src/lib/ipc.youtube.test.ts`

**Interfaces:**
- Consumes: `@tauri-apps/api/core`, `@tauri-apps/api/event`
- Produces:
  - `checkYtDlpStatus()`
  - `ensureYtDlp()`
  - `inspectYoutubeUrl(url: string)`
  - `downloadYoutubeItems(req: YoutubeDownloadRequest)`
  - `cancelYoutubeDownload()`
  - `listenYoutubeProgress(callback)`

- [ ] **Step 1: Write test for IPC YouTube functions**

```typescript
// ui/src/lib/ipc.youtube.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as ipc from './ipc';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => true),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

describe('YouTube IPC API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('inspectYoutubeUrl invokes backend command', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    (invoke as any).mockResolvedValueOnce({
      is_playlist: false,
      title: 'Demo Video',
      items: [{ id: '123', title: 'Demo Video', webpage_url: 'https://youtube.com/watch?v=123' }],
    });

    const res = await ipc.inspectYoutubeUrl('https://youtube.com/watch?v=123');
    expect(invoke).toHaveBeenCalledWith('inspect_youtube_url', { url: 'https://youtube.com/watch?v=123' });
    expect(res.title).toBe('Demo Video');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test ui/src/lib/ipc.youtube.test.ts`
Expected: FAIL (`inspectYoutubeUrl` not exported)

- [ ] **Step 3: Implement types and IPC functions**

Create `ui/src/types/youtube.ts` with interfaces from design.
Export from `ui/src/types/index.ts`.
In `ui/src/lib/ipc.ts`:
- Add `checkYtDlpStatus`, `ensureYtDlp`, `inspectYoutubeUrl`, `downloadYoutubeItems`, `cancelYoutubeDownload`, and `listenYoutubeProgress`.

- [ ] **Step 4: Run test to verify it passes**

Run: `npm --prefix ui test ui/src/lib/ipc.youtube.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/types/youtube.ts ui/src/lib/ipc.ts ui/src/lib/ipc.youtube.test.ts
git commit -m "feat(ui): add YouTube TypeScript interfaces and IPC methods"
```

---

### Task 5: `YoutubeImportModal` Component & Unit Tests

**Files:**
- Create: `ui/src/components/YoutubeImportModal.tsx`
- Test: `ui/src/components/YoutubeImportModal.test.tsx`

**Interfaces:**
- Props:
  - `isOpen: boolean`
  - `onClose: () => void`
  - `onVideosDownloaded: (savedPaths: string[]) => void`
  - `defaultDownloadDir?: string`

- [ ] **Step 1: Write tests for `YoutubeImportModal`**

```typescript
// ui/src/components/YoutubeImportModal.test.tsx
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { YoutubeImportModal } from './YoutubeImportModal';
import * as ipc from '../lib/ipc';

vi.mock('../lib/ipc', () => ({
  inspectYoutubeUrl: vi.fn(),
  downloadYoutubeItems: vi.fn(),
  cancelYoutubeDownload: vi.fn(),
  listenYoutubeProgress: vi.fn(() => Promise.resolve(() => {})),
}));

describe('YoutubeImportModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders input field and inspect button when open', () => {
    render(<YoutubeImportModal isOpen={true} onClose={vi.fn()} onVideosDownloaded={vi.fn()} />);
    expect(screen.getByPlaceholderText(/dán link youtube/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /kiểm tra/i })).toBeInTheDocument();
  });

  it('inspects playlist and toggles selection', async () => {
    (ipc.inspectYoutubeUrl as any).mockResolvedValueOnce({
      is_playlist: true,
      playlist_id: 'PL1',
      title: 'Sample Course',
      items: [
        { id: 'v1', title: 'Lesson 1', duration_seconds: 120, webpage_url: 'url1' },
        { id: 'v2', title: 'Lesson 2', duration_seconds: 180, webpage_url: 'url2' },
      ],
    });

    render(<YoutubeImportModal isOpen={true} onClose={vi.fn()} onVideosDownloaded={vi.fn()} />);
    const input = screen.getByPlaceholderText(/dán link youtube/i);
    fireEvent.change(input, { target: { value: 'https://youtube.com/playlist?list=PL1' } });
    fireEvent.click(screen.getByRole('button', { name: /kiểm tra/i }));

    await waitFor(() => {
      expect(screen.getByText('Sample Course')).toBeInTheDocument();
      expect(screen.getByText('Lesson 1')).toBeInTheDocument();
      expect(screen.getByText('Lesson 2')).toBeInTheDocument();
    });

    // Check action button shows count
    expect(screen.getByText(/tải 2 video đã chọn/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test ui/src/components/YoutubeImportModal.test.tsx`
Expected: FAIL (`YoutubeImportModal` not created)

- [ ] **Step 3: Implement `YoutubeImportModal.tsx`**

Implement `ui/src/components/YoutubeImportModal.tsx`:
- URL input with paste button.
- Inspect state: validation, error banner, inspect spinner.
- Single video view: Thumbnail, title, duration, "Tải video" button.
- Playlist view: Select All / Deselect All, individual item checkboxes, selected count badge, "Tải X video đã chọn" button.
- Download progress state: overall progress %, individual items status, speed/ETA display, "Hủy tải" and "Chạy ngầm" buttons.
- Call `onVideosDownloaded([path])` when items complete.

- [ ] **Step 4: Run test to verify it passes**

Run: `npm --prefix ui test ui/src/components/YoutubeImportModal.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/YoutubeImportModal.tsx ui/src/components/YoutubeImportModal.test.tsx
git commit -m "feat(ui): add YoutubeImportModal component with inspection and progress tracking"
```

---

### Task 6: UI Integration — HeaderBar, Empty State & JobList Queueing

**Files:**
- Modify: `ui/src/components/HeaderBar.tsx`
- Modify: `ui/src/components/VideoPlayer.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/components/HeaderBar.test.tsx`

**Interfaces:**
- `HeaderBar`: Add `onOpenYoutubeModal: () => void` prop and button with Youtube icon.
- `VideoPlayer`: Add button in empty state to open YouTube modal.
- `App.tsx`: Manage `isYoutubeModalOpen` state and feed downloaded paths to `addJobs(paths)`.

- [ ] **Step 1: Write test for HeaderBar YouTube button**

Update `ui/src/components/HeaderBar.test.tsx` to verify that the YouTube import button renders and triggers `onOpenYoutubeModal`.

- [ ] **Step 2: Run test to verify failure**

Run: `npm --prefix ui test ui/src/components/HeaderBar.test.tsx`
Expected: FAIL

- [ ] **Step 3: Implement modifications in HeaderBar, VideoPlayer, and App**

1. In `HeaderBar.tsx`:
   - Add `onOpenYoutubeModal: () => void` to `HeaderBarProps`.
   - Add button beside `onSelectVideo` with `Youtube` icon and label `"Nhập từ YouTube"`.
2. In `VideoPlayer.tsx`:
   - In the empty state view, add a secondary button: `"Hoặc dán đường link YouTube để tải video / playlist"`.
3. In `App.tsx`:
   - Add `const [isYoutubeModalOpen, setIsYoutubeModalOpen] = useState(false)`.
   - Pass `onOpenYoutubeModal={() => setIsYoutubeModalOpen(true)}` to `HeaderBar` and `VideoPlayer`.
   - Render `<YoutubeImportModal isOpen={isYoutubeModalOpen} onClose={() => setIsYoutubeModalOpen(false)} onVideosDownloaded={handleVideosDownloaded} />`.
   - `handleVideosDownloaded` calls `addJobs(paths)` and shows a toast: `"Đã thêm X video từ YouTube vào hàng đợi"`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm --prefix ui test`
Expected: PASS all tests.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/HeaderBar.tsx ui/src/components/VideoPlayer.tsx ui/src/App.tsx ui/src/components/HeaderBar.test.tsx
git commit -m "feat(ui): integrate YouTube modal into HeaderBar and VideoPlayer empty state"
```

---

### Task 7: Download Folder Configuration in Settings

**Files:**
- Modify: `src/settings/model.rs`
- Modify: `ui/src/components/SettingsModal.tsx`
- Modify: `ui/src/types/index.ts`
- Test: `tests/settings_model_test.rs`
- Test: `ui/src/components/SettingsModal.test.tsx`

**Interfaces:**
- Add `download_dir: Option<String>` to `GeneralSettings`.
- `SettingsModal`: add directory picker input for "Thư mục lưu video tải về" in General settings tab.

- [ ] **Step 1: Add unit test for `download_dir` in settings**

In `tests/settings_model_test.rs`:
Test that `Settings` defaults `download_dir` to `None` (which defaults dynamically to `~/Videos/auto-subtitle/downloads/`).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test settings_model_test`
Expected: FAIL

- [ ] **Step 3: Implement settings model and UI field**

1. In `src/settings/model.rs`:
   - Add `#[serde(default)] pub download_dir: Option<String>` to `GeneralSettings`.
2. In `ui/src/types/index.ts`:
   - Add `download_dir?: string` to `GeneralSettings`.
3. In `ui/src/components/SettingsModal.tsx`:
   - Add input for "Thư mục lưu video tải về" with folder browse button in the General Tab.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test settings_model_test` and `npm --prefix ui test ui/src/components/SettingsModal.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/settings/model.rs ui/src/types/index.ts ui/src/components/SettingsModal.tsx tests/settings_model_test.rs
git commit -m "feat(settings): add download_dir setting for YouTube downloads"
```
