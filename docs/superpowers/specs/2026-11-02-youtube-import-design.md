# YouTube Video & Playlist Import — Design

## Goal

Allow users to import videos directly from YouTube by pasting a URL (single video or playlist) instead of only picking local files. The app inspects video/playlist metadata, lets users choose items, downloads high-quality MP4 files locally via a managed `yt-dlp` binary, and seamlessly adds them to the existing `JobList` for automatic subtitle generation and translation.

## Non-goals

- Downloading YouTube auto-generated captions/subtitles directly instead of running Whisper (focus remains on high-accuracy STT with Whisper + LLM translation).
- Downloading 4K/8K formats or complex codec configurations (standard 1080p/720p H.264/AAC MP4 is targeted for fast preview and maximum player compatibility).
- Live-stream capture or real-time continuous recording.
- General audio-only conversion pipeline during download (all downloads are packaged as MP4 video files with audio).

## 1. System Architecture & Binary Management

```
┌─────────────────────────────────────────────────────────────────┐
│                          React Frontend                         │
│  ┌───────────────────────┐         ┌─────────────────────────┐  │
│  │  HeaderBar / Player   │         │  YoutubeImportModal     │  │
│  │  [Nhập từ YouTube] ───┼────────►│  - Input URL            │  │
│  └───────────────────────┘         │  - Inspect metadata     │  │
│                                    │  - Playlist selection   │  │
│                                    │  - Progress tracking    │  │
│                                    └────────────┬────────────┘  │
└─────────────────────────────────────────────────┼───────────────┘
                                                  │ Tauri IPC
                                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Tauri / Rust Backend                       │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ Tauri Commands:                                           │  │
│  │ - check_yt_dlp_status()                                   │  │
│  │ - ensure_yt_dlp()                                         │  │
│  │ - inspect_youtube_url(url)                                │  │
│  │ - download_youtube_items(req)                             │  │
│  │ - cancel_youtube_download(task_id)                        │  │
│  └─────────────────────────────┬─────────────────────────────┘  │
│                                │                                │
│                                ▼                                │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │ YtDlpManager:                                             │  │
│  │ - Locate binary (PATH -> App Data Directory)              │  │
│  │ - Auto-download from GitHub releases if missing           │  │
│  │ - Execute inspect command (--dump-single-json)            │  │
│  │ - Execute stream download with progress parser            │  │
│  └─────────────────────────────┬─────────────────────────────┘  │
└────────────────────────────────┼────────────────────────────────┘
                                 │ Process Spawn
                                 ▼
                     ┌───────────────────────┐
                     │     yt-dlp binary     │
                     └───────────────────────┘
```

### 1.1 `yt-dlp` Binary Discovery & Installation
- Binary location lookup precedence:
  1. System `PATH` (`which::which("yt-dlp")`).
  2. Application data directory:
     - Linux: `~/.local/share/auto-subtitle/bin/yt-dlp`
     - Windows: `%LOCALAPPDATA%\auto-subtitle\bin\yt-dlp.exe`
     - macOS: `~/Library/Application Support/auto-subtitle/bin/yt-dlp`
- Auto-download binary from official GitHub releases if not present:
  - Linux x86_64: `https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp`
  - macOS (universal): `https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos`
  - Windows: `https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe`
  - Unix permissions: automatically execute `chmod +x` after download.

### 1.2 Output Directory & File Naming
- Default download folder: `~/Videos/auto-subtitle/downloads/` (fallback to system Downloads or AppData directory).
- Configurable via `Settings.general.download_dir` in `SettingsModal`.
- Output file template: `%(title).150B [%(id)s].%(ext)s`.
- Output format: `-f "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/b" --merge-output-format mp4` (ensures H.264/AAC MP4 compatible with HTML5 `<video>` preview).

---

## 2. Data Models (Rust & TypeScript)

### 2.1 Rust Data Models (`src/youtube/model.rs` & `src-tauri/src/types.rs`)

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
    pub stage: String, // "downloading" | "merging" | "completed" | "error"
    pub percentage: f32,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub saved_path: Option<String>,
    pub error: Option<String>,
}
```

### 2.2 TypeScript Definitions (`ui/src/types/youtube.ts`)

```typescript
export interface YoutubeVideoItem {
  id: string;
  title: string;
  duration_seconds?: number;
  thumbnail_url?: string;
  channel_name?: string;
  webpage_url: string;
}

export interface YoutubeInspectResult {
  is_playlist: boolean;
  playlist_id?: string;
  title: string;
  items: YoutubeVideoItem[];
}

export interface YoutubeProgressPayload {
  video_id: string;
  index: number;
  total_items: number;
  stage: 'downloading' | 'merging' | 'completed' | 'error';
  percentage: number;
  speed?: string;
  eta?: string;
  saved_path?: string;
  error?: string;
}
```

---

## 3. Tauri IPC Commands & Events

### 3.1 Commands
1. `check_yt_dlp_status() -> Result<YtDlpStatus, String>`
   - Checks if `yt-dlp` executable exists and queries `yt-dlp --version`.
2. `ensure_yt_dlp() -> Result<YtDlpStatus, String>`
   - Downloads `yt-dlp` binary if missing, marks executable, and returns status.
3. `inspect_youtube_url(url: String) -> Result<YoutubeInspectResult, String>`
   - Executes `yt-dlp --dump-single-json --flat-playlist --no-warnings <url>`.
   - Parses the JSON output:
     - If `_type == "playlist"`, extract array of entries into `items`.
     - If single video, wrap single entry into `items`.
4. `download_youtube_items(app_handle: AppHandle, req: YoutubeDownloadRequest) -> Result<(), String>`
   - Initiates sequential downloads of requested items.
   - For each item:
     - Check if output file already exists. If yes, immediately emit `completed` with existing path.
     - Spawns `yt-dlp` with `--newline --progress-template "%(progress.downloaded_bytes)d %(progress.total_bytes)d %(progress.speed)f %(progress.eta)d"`.
     - Emits `youtube-download-progress` event to frontend on stdout lines.
     - Upon completion, emits `completed` with `saved_path`.
5. `cancel_youtube_download() -> Result<(), String>`
   - Terminates active child process and clears the download queue.

### 3.2 Events
- Event name: `youtube-download-progress`
- Payload: `YoutubeProgressPayload`

---

## 4. UI & UX Design

### 4.1 Entry Points
1. **HeaderBar**:
   - Add button `"Nhập từ YouTube"` next to `"Mở video..."` with YouTube icon (`lucide-react: Youtube`).
2. **VideoPlayer Empty State**:
   - Add clickable secondary button: *"Hoặc dán đường link YouTube để tải video / playlist"*.

### 4.2 `YoutubeImportModal.tsx`
- **Stage 1: Input & Inspect**
  - Text input for URL with paste button (`Paste` icon) and auto-detect YouTube link on paste.
  - "Kiểm tra" button with loading spinner (`Loader2`).
  - Validation error display (invalid URL, network error, private video).
- **Stage 2: Preview & Selection**
  - **Single video**: Card preview showing thumbnail (16:9), video title, channel name, formatted duration (`mm:ss` or `hh:mm:ss`), and `"Tải video"` primary button.
  - **Playlist**:
    - Header showing playlist title, total video count, and selected count (`Đã chọn X/Y video`).
    - Select controls: `"Chọn tất cả"` / `"Bỏ chọn tất cả"` buttons.
    - Scrollable list with checkboxes, index numbers, thumbnails, titles, and durations.
    - Primary action button: `"Tải X video đã chọn"`.
- **Stage 3: Download Tracking**
  - Overall progress bar (e.g. `2/5 video hoàn tất`).
  - Current item progress bar with real-time percentage, download speed (`MB/s`), and ETA.
  - Action buttons:
    - `"Hủy"`: Cancels remaining downloads.
    - `"Để tải ngầm"` / Close: Closes modal while downloads continue in background.
  - Integration with `JobList`: As each video finishes, `addJobs([saved_path])` is invoked; toast notification `"Đã tải xong: [Tên video]"` appears.

### 4.3 Settings Configuration (`SettingsModal.tsx`)
- General Tab additions:
  - Input field for **"Thư mục lưu video tải về"** (`download_dir`).
  - Button **"Chọn thư mục..."** calling native directory picker.
  - Status indicator for `yt-dlp` (Installed / Version / Re-download button).

---

## 5. Error Handling & Edge Cases

| Scenario | Handling Strategy |
|---|---|
| **Private or Deleted Video** | Inspect returns error message; UI displays clear message: "Video không tồn tại hoặc đã bị đặt ở chế độ riêng tư". |
| **Age-restricted Video** | `yt-dlp` reports login requirement; UI advises user or fails gracefully with "Video yêu cầu xác minh độ tuổi". |
| **Combined URL (`watch?v=...&list=...`)** | Detect both `v=` and `list=`; prompt user with choice: "Tải video hiện tại" hoặc "Tải toàn bộ playlist". |
| **Network Interruption** | `yt-dlp` retries natively with partial `.part` file; on persistent failure, mark item as "Lỗi" and proceed to next playlist item without crashing. |
| **File Already Downloaded** | Check filesystem before spawning `yt-dlp`; if complete MP4 exists, skip re-download and add directly to `JobList`. |
| **Missing yt-dlp binary** | Automatically prompt and trigger `ensure_yt_dlp()` with inline progress bar. |

---

## 6. Testing Strategy

1. **Rust Unit & Integration Tests**:
   - URL parsing tests: Validate identification of single videos, playlists, Shorts, and composite URLs.
   - Metadata JSON parsing tests: Deserialize sample `yt-dlp --dump-single-json` payloads for both single videos and playlists.
   - Binary path resolution test: Verify fallback order from system PATH to app data directory.
2. **Frontend Component Tests (`YoutubeImportModal.test.tsx`)**:
   - Input validation: Test valid/invalid YouTube URL formats.
   - Inspect state rendering: Check spinner during inspection and error banner upon inspection failure.
   - Playlist item selection: Test "Select All", "Deselect All", and individual checkbox toggles.
   - Progress and events: Mock Tauri `listen('youtube-download-progress')` and verify UI progress updates.
   - Job queueing: Verify `addJobs` is triggered with output path when a download completes.
