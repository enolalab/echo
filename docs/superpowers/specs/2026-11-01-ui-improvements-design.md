# Design Document: Echo Studio — Cải tiến UI (Sub-project C)

- **Date:** 2026-11-01
- **Project:** `auto-subtitle` (Echo Studio)
- **Scope:** Frontend only (`ui/`). Không sửa Rust, không thêm dependency mới.
- **Status:** Draft — chờ duyệt

---

## 1. Tổng quan

Bốn cải tiến cho giao diện desktop hiện có:

1. **Kéo-thả file** — kéo video/audio từ file manager vào cửa sổ app để thêm vào hàng đợi.
2. **Resize panel** — kéo ranh giới giữa cột video và cột editor, và giữa video với danh sách job.
3. **Timeline seekbar** — thanh seek dưới video hiển thị block vị trí các đoạn phụ đề (timeline đơn giản, KHÔNG phải waveform âm thanh thật).
4. **Polish** — toast thông báo, empty states đẹp hơn, spacing nhất quán.

**Ràng buộc đã chốt:** zero dependency mới — mọi thứ tự viết (splitter, toast, drop handler, seekbar).

---

## 2. Resize panel — `ui/src/components/SplitPane.tsx`

```tsx
interface SplitPaneProps {
  direction: 'horizontal' | 'vertical';  // horizontal = trái|phải, vertical = trên|dưới
  initialRatio: number;                // 0–1, tỉ lệ phần panel A
  minA: number;                        // px tối thiểu phía A
  minB: number;                        // px tối thiểu phía B
  storageKey?: string;                 // có → persist ratio vào localStorage "echo.layout.<key>"
  children: [ReactNode, ReactNode];
}
```

**Cơ chế:**
- Container `flex` đầy khung; panel A có `flexBasis: ${ratio * 100}%`, panel B chiếm phần còn lại; divider rộng 5px `cursor: col-resize` (hoặc `row-resize`), `shrink-0`.
- `onMouseDown` trên divider → đăng ký `mousemove` + `mouseup` trên `document`; tính ratio từ vị trí con trỏ so với `getBoundingClientRect()` của container; clamp để cả hai phía ≥ min px (`minA/containerSize ≤ ratio ≤ 1 - minB/containerSize`).
- Trong lúc kéo: thêm class chặn text-selection lên `document.body`, gỡ khi mouseup. Listener dọn sạch trong `useEffect` cleanup.
- Persist: lúc mount đọc `localStorage.getItem('echo.layout.<storageKey>')`; nếu parse được số hợp lệ (0–1) thì dùng thay `initialRatio`; ghi lại mỗi lần kéo xong (mouseup).

**Áp dụng ở `App.tsx`:**
- `<main>` hiện `grid grid-cols-1 md:grid-cols-2` → `<SplitPane direction="horizontal" initialRatio={0.5} minA={420} minB={360} storageKey="main">`. Bỏ hẳn responsive stacking — app desktop, min-width cửa sổ trong `tauri.conf.json` đã đủ lớn.
- Cột trái (video `flex-1` + danh sách job `h-44` cố định) → `<SplitPane direction="vertical" initialRatio={0.7} minA={240} minB={120} storageKey="video-list">`.

**Edge case:** cửa sổ nhỏ hơn `minA + minB` → panel A giữ `minA`, panel B co xuống dưới `minB` (không âm), tức clamp ưu tiên phía A. Ratio lưu trong localStorage bị hỏng → fallback `initialRatio`.

---

## 3. Kéo-thả file — `ui/src/hooks/useFileDrop.ts`

```ts
export function useFileDrop(onDrop: (paths: string[]) => void): { isDragging: boolean }
```

**Tauri (chính):** `getCurrentWindow().onDragDropEvent((e) => …)`:
- `enter`/`over` → `setIsDragging(true)`
- `leave` → `setIsDragging(false)`
- `drop` → `setIsDragging(false)`, lọc `e.payload.paths` theo extension, gọi `onDrop(filtered)` nếu còn phần tử.

**Browser dev fallback** (khi `!isTauri()`): lắng `dragover`/`dragleave`/`drop` trên `document`. `drop` → `e.dataTransfer.files` lọc extension, map sang `URL.createObjectURL(f)` (giống `handleSelectVideos` hiện tại) — video blob URL dùng preview được nhưng pipeline thật chỉ chạy trong Tauri, browser chỉ là dev mode.

**Extension hợp lệ:** `mp4 mkv mov avi webm m4v mp3 wav flac m4a ogg` (lowercase, so trước dấu `.` cuối).

**Overlay:** khi `isDragging` render lớp phủ toàn cửa sổ trong `App.tsx`: `fixed inset-0 z-40 bg-blue-950/60 border-2 border-dashed border-blue-400`, icon `FileVideo`, text "Thả để thêm vào hàng đợi", `pointer-events-none` (không nuốt event drop của Tauri/browser).

**Hành vi:** `onDrop` gọi `addJobs` có sẵn (file trùng vẫn thêm — nhất quán với file picker). Kéo thả lúc `isRunning` vẫn nhận — chỉ thêm vào hàng đợi. Tất cả file bị loại bởi filter → toast lỗi "Không có file video/audio hợp lệ".

---

## 4. Timeline seekbar — `ui/src/components/SeekBar.tsx`

```tsx
interface SeekBarProps {
  durationMs: number;
  segments: SubtitleSegment[];
  currentTimeMs: number;
  onSeek: (ms: number) => void;
}
```

**Render:** một track `div` cao ~10px, nền `bg-slate-700`, `rounded`, `relative`, `cursor-pointer`:
- Mỗi segment = block absolute: `left: ${(start_ms / durationMs) * 100}%`, `width: ${((end_ms - start_ms) / durationMs) * 100}%` — clamp `[0, 100]`. Màu `bg-slate-500/70`; segment đang chứa `currentTimeMs` → `bg-blue-400`.
- Playhead: vạch 2px `bg-blue-300` tại `left: ${(currentTimeMs / durationMs) * 100}%`.
- Click vào track → `ms = ((e.clientX - rect.left) / rect.width) * durationMs`, clamp `[0, durationMs]` → `onSeek(ms)`.
- `durationMs <= 0` → render track rỗng, click bị bỏ qua.

**Tích hợp:** `SeekBar` **thay thế** `<input type="range">` hiện có trong control bar của `VideoPlayer.tsx` (dòng ~256–266). `VideoPlayer` đã có sẵn `durationMs`, `segments`, `currentTimeMs` — `onSeek` đặt `videoRef.current.currentTime = ms / 1000` và gọi `onTimeUpdate(ms)`, dùng lại `handleSliderSeek` hiện có (đổi signature nhận `number`). Không cần prop mới lên `App`.

**Edge case:** segment ngoài `[0, durationMs]` (do sửa tay) → clamp vào khung nhìn. Segment 0-width → tối thiểu 2px để còn nhìn thấy.

---

## 5. Polish

### 5.1 Toast — `ui/src/components/Toasts.tsx`

```ts
export interface Toast { id: number; text: string; kind: 'info' | 'success' | 'error' }
```

- State `toasts: Toast[]` ở `App.tsx`; helper `pushToast(text, kind)` tạo id tăng dần, `setTimeout(4000)` tự gỡ. Render `fixed bottom-4 right-4 z-50 flex flex-col gap-2`, tối đa 4 toast cùng lúc (bỏ cái cũ nhất khi thừa). Mỗi toast: `rounded-md px-3 py-2 text-xs shadow-lg` + màu theo kind (`info` slate, `success` emerald, `error` rose) + nút `×` đóng tay.
- Điểm gọi (bổ sung, KHÔNG bỏ log trong drawer):
  - `runJob` catch → `pushToast(msg, 'error')`; job `done` → `pushToast("Hoàn tất <tên file>", 'success')`.
  - `handleSelectProfile` / `handleSelectSkill` catch → toast lỗi.
  - Drop toàn file không hợp lệ → toast lỗi (mục 3).
  - `load_warning` khi mở app → toast `info` (vẫn giữ dòng log `[CẢNH BÁO]`).

### 5.2 Empty states

- `JobList` rỗng: giữ text, thêm icon `FileVideo` mờ + gợi ý "Kéo thả file vào cửa sổ".
- `SubtitleEditor` rỗng: thống nhất pattern icon + một dòng hướng dẫn (xem file hiện tại, chỉ chỉnh text nếu thiếu icon).
- `VideoPlayer` không có video: giữ nguyên (đã có icon + text kéo-thả đúng ý).

### 5.3 Nhất quán nhỏ

- Chuẩn `border-slate-800` cho panel lớn, `border-slate-700` cho control nhỏ; `rounded-lg` cho panel, `rounded-md` cho control. Chỉ sửa chỗ đang lệch rõ rệt — không refactor styling tổng.

---

## 6. Xử lý lỗi

| Tình huống | Hành vi |
|---|---|
| File thả vào sai định dạng | Bỏ qua; nếu loại hết → toast "Không có file video/audio hợp lệ" |
| `localStorage` layout hỏng/không có | Fallback `initialRatio` |
| `durationMs` = 0 (metadata chưa load) | SeekBar track rỗng, không seek được |
| Cửa sổ quá nhỏ cho min panel | Ưu tiên panel A giữ `minA` |
| `onDragDropEvent` lỗi (hiếm) | `console.warn`, app vẫn dùng file picker |

---

## 7. Testing (Vitest + Testing Library)

- **`SplitPane.test.tsx`:** mock `getBoundingClientRect` của container; `fireEvent.mouseDown` trên divider → `mouseMove(document, x)` → `mouseUp` → assert `flexBasis` panel A đổi đúng; kéo ra ngoài min → clamp; `storageKey` ghi/đọc `localStorage`.
- **`useFileDrop.test.ts` (hoặc `.tsx` với `renderHook`):** mock `@tauri-apps/api/window` → `getCurrentWindow().onDragDropEvent` bắt callback; giả lập `over` → `isDragging` true; `drop` với `paths` lẫn file `.txt` → `onDrop` chỉ nhận file hợp lệ; `leave` → `isDragging` false.
- **`SeekBar.test.tsx`:** mock `getBoundingClientRect` của track; `durationMs=10000`, click tại 50% → `onSeek(5000)`; block segment hiện đúng `style.left/width`; segment active đổi màu.
- **`Toasts.test.tsx`:** mount → `pushToast` → text hiện; `vi.useFakeTimers` tua 4s → biến mất; nút `×` đóng ngay.
- **`App.test.tsx` (bổ sung nhẹ):** mở app → `fireEvent(document, new Event('dragover'))` → overlay "Thả để thêm vào hàng đợi" hiện. Trong jsdom `isTauri()` = false nên hook đi đường document listener — test case này deterministic, không cần mock `@tauri-apps/api/window`.

---

## 8. File thay đổi

**Tạo mới:**
- `ui/src/components/SplitPane.tsx` + `SplitPane.test.tsx`
- `ui/src/components/SeekBar.tsx` + `SeekBar.test.tsx`
- `ui/src/components/Toasts.tsx` + `Toasts.test.tsx`
- `ui/src/hooks/useFileDrop.ts` + `useFileDrop.test.ts`

**Sửa:**
- `ui/src/App.tsx` — SplitPane layout, useFileDrop + overlay, toast state + điểm gọi, `App.test.tsx` (thêm case overlay).
- `ui/src/components/VideoPlayer.tsx` — đổi range input thành `SeekBar`.
- `ui/src/components/JobList.tsx` — empty state.
- `ui/src/components/SubtitleEditor.tsx` — empty state (nếu thiếu icon).

---

## 9. Ngoài phạm vi

- Waveform âm thanh thật (đã chốt: timeline đơn giản).
- Đổi layout tổng / theme mới / responsive mobile.
- Backend/Rust/IPC mới — kéo-thả dùng sẵn `onDragDropEvent` của cửa sổ Tauri, không cần command.
- Dependency mới (`react-resizable-panels`, `sonner`, …) — đã chốt zero-dep.
