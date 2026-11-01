# UI Improvements (Sub-project C) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Kéo-thả file vào app, resize được các panel chính, timeline seekbar hiển thị block phụ đề, và polish UI (toast + empty states).

**Architecture:** Toàn bộ trong `ui/` — zero dependency mới, không sửa Rust. `SplitPane` và `SeekBar` là component tự viết; `useFileDrop` bọc `onDragDropEvent` của Tauri với fallback document listeners; `useToasts` + `Toasts` quản lý thông báo tự đóng.

**Tech Stack:** React 18, TypeScript, Tailwind, Vitest + Testing Library, Tauri 2 (`@tauri-apps/api/window`).

**Spec:** `docs/superpowers/specs/2026-11-01-ui-improvements-design.md`

## Global Constraints

- KHÔNG thêm dependency mới vào `ui/package.json`.
- KHÔNG sửa file Rust (`src/`, `src-tauri/`).
- Mọi string hiển thị cho user bằng tiếng Việt.
- Test command: `npm --prefix ui test`. Build: `npm --prefix ui run build`.
- `tauri.conf.json` không có `dragDropEnabled` → mặc định `true` trong Tauri 2, kéo-thả dùng ngay.
- Commit message theo style có sẵn: `feat(ui): ...` / `test(ui): ...`.

---

### Task 1: SplitPane component

**Files:**
- Create: `ui/src/components/SplitPane.tsx`
- Test: `ui/src/components/SplitPane.test.tsx`

**Interfaces:**
- Produces: `SplitPane` — props `{ direction: 'horizontal' | 'vertical'; initialRatio: number; minA: number; minB: number; storageKey?: string; children: [ReactNode, ReactNode] }`. Task 5 mount nó vào layout `App.tsx`.

- [ ] **Step 1: Write the failing test** — `ui/src/components/SplitPane.test.tsx`

```tsx
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { SplitPane } from './SplitPane';

const RECT = { left: 0, top: 0, width: 1000, height: 1000, right: 1000, bottom: 1000, x: 0, y: 0, toJSON: () => {} } as DOMRect;

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(RECT);
  localStorage.clear();
});
afterEach(() => vi.restoreAllMocks());

const renderPane = (props: Partial<React.ComponentProps<typeof SplitPane>> = {}) =>
  render(
    <SplitPane direction="horizontal" initialRatio={0.5} minA={100} minB={100} {...props}>
      <div>A</div>
      <div>B</div>
    </SplitPane>
  );

describe('SplitPane', () => {
  it('drags the divider and updates ratio with min clamp', () => {
    renderPane();
    const sep = screen.getByRole('separator');
    const panelA = screen.getByTestId('split-a');
    expect(panelA.style.flexBasis).toBe('50%');

    fireEvent.mouseDown(sep);
    fireEvent.mouseMove(document, { clientX: 700 });
    expect(panelA.style.flexBasis).toBe('70%');

    // drag beyond minA (100px / 1000px = 10%) → clamps to 10%
    fireEvent.mouseMove(document, { clientX: 50 });
    expect(panelA.style.flexBasis).toBe('10%');
    fireEvent.mouseUp(document);
  });

  it('persists ratio to localStorage on mouseup when storageKey set', () => {
    renderPane({ storageKey: 'main' });
    fireEvent.mouseDown(screen.getByRole('separator'));
    fireEvent.mouseMove(document, { clientX: 700 });
    fireEvent.mouseUp(document);
    expect(localStorage.getItem('echo.layout.main')).toBe('0.7');
  });

  it('loads persisted ratio on mount', () => {
    localStorage.setItem('echo.layout.main', '0.3');
    renderPane({ storageKey: 'main' });
    expect(screen.getByTestId('split-a').style.flexBasis).toBe('30%');
  });

  it('falls back to initialRatio on corrupted storage', () => {
    localStorage.setItem('echo.layout.main', 'garbage');
    renderPane({ storageKey: 'main' });
    expect(screen.getByTestId('split-a').style.flexBasis).toBe('50%');
  });

  it('vertical direction uses clientY', () => {
    renderPane({ direction: 'vertical', initialRatio: 0.5 });
    fireEvent.mouseDown(screen.getByRole('separator'));
    fireEvent.mouseMove(document, { clientY: 800 });
    expect(screen.getByTestId('split-a').style.flexBasis).toBe('80%');
    fireEvent.mouseUp(document);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/components/SplitPane.test.tsx`
Expected: FAIL — `./SplitPane` not found.

- [ ] **Step 3: Implement** — `ui/src/components/SplitPane.tsx`

```tsx
import React, { useCallback, useRef, useState } from 'react';

interface SplitPaneProps {
  direction: 'horizontal' | 'vertical';
  initialRatio: number;
  minA: number;
  minB: number;
  storageKey?: string;
  children: [React.ReactNode, React.ReactNode];
}

function loadRatio(storageKey: string | undefined, fallback: number): number {
  if (!storageKey) return fallback;
  try {
    const raw = localStorage.getItem(`echo.layout.${storageKey}`);
    const v = raw === null ? NaN : Number(raw);
    return Number.isFinite(v) && v > 0 && v < 1 ? v : fallback;
  } catch {
    return fallback;
  }
}

export const SplitPane: React.FC<SplitPaneProps> = ({
  direction,
  initialRatio,
  minA,
  minB,
  storageKey,
  children,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [ratio, setRatio] = useState(() => loadRatio(storageKey, initialRatio));
  const horizontal = direction === 'horizontal';

  const startDrag = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const el = containerRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const size = horizontal ? rect.width : rect.height;
      if (size <= 0) return;
      const start = horizontal ? rect.left : rect.top;

      document.body.style.userSelect = 'none';
      document.body.style.cursor = horizontal ? 'col-resize' : 'row-resize';

      const onMove = (ev: MouseEvent) => {
        const pos = (horizontal ? ev.clientX : ev.clientY) - start;
        const minR = minA / size;
        const maxR = 1 - minB / size;
        setRatio(Math.min(Math.max(pos / size, minR), Math.max(minR, maxR)));
      };
      const onUp = () => {
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
        document.body.style.userSelect = '';
        document.body.style.cursor = '';
        if (storageKey) {
          setRatio((r) => {
            try {
              localStorage.setItem(`echo.layout.${storageKey}`, String(r));
            } catch {
              /* ignore */
            }
            return r;
          });
        }
      };
      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    },
    [horizontal, minA, minB, storageKey]
  );

  return (
    <div
      ref={containerRef}
      className={`flex ${horizontal ? 'flex-row' : 'flex-col'} h-full w-full min-h-0 min-w-0`}
    >
      <div
        data-testid="split-a"
        style={{ flexBasis: `${ratio * 100}%` }}
        className="shrink-0 min-w-0 min-h-0 overflow-hidden"
      >
        {children[0]}
      </div>
      <div
        role="separator"
        aria-orientation={horizontal ? 'vertical' : 'horizontal'}
        onMouseDown={startDrag}
        className={`shrink-0 ${
          horizontal ? 'w-[5px] cursor-col-resize' : 'h-[5px] cursor-row-resize'
        } bg-slate-800 hover:bg-blue-600/50 transition-colors`}
      />
      <div className="flex-1 min-w-0 min-h-0 overflow-hidden">{children[1]}</div>
    </div>
  );
};
```

- [ ] **Step 4: Run tests**

Run: `npm --prefix ui test -- src/components/SplitPane.test.tsx`
Expected: 5/5 PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/SplitPane.tsx ui/src/components/SplitPane.test.tsx
git commit -m "feat(ui): SplitPane component with drag-to-resize and persisted ratio"
```

---

### Task 2: SeekBar component + VideoPlayer integration

**Files:**
- Create: `ui/src/components/SeekBar.tsx`
- Test: `ui/src/components/SeekBar.test.tsx`
- Modify: `ui/src/components/VideoPlayer.tsx` (đổi `<input type="range">` thành `SeekBar`)

**Interfaces:**
- Consumes: `SubtitleSegment` từ `../types` (`{ id: number; start_ms: number; end_ms: number; ... }`).
- Produces: `SeekBar` — props `{ durationMs: number; segments: SubtitleSegment[]; currentTimeMs: number; onSeek: (ms: number) => void }`.

- [ ] **Step 1: Write the failing test** — `ui/src/components/SeekBar.test.tsx`

```tsx
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { SeekBar } from './SeekBar';
import type { SubtitleSegment } from '../types';

const RECT = { left: 0, top: 0, width: 400, height: 10, right: 400, bottom: 10, x: 0, y: 0, toJSON: () => {} } as DOMRect;
const seg: SubtitleSegment = { id: 1, start_ms: 1000, end_ms: 3000, original_text: 'a', translated_text: 'b' };

afterEach(() => vi.restoreAllMocks());

describe('SeekBar', () => {
  it('click seeks to the right ms', () => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(RECT);
    const onSeek = vi.fn();
    render(<SeekBar durationMs={10000} segments={[]} currentTimeMs={0} onSeek={onSeek} />);
    fireEvent.click(screen.getByRole('slider'), { clientX: 200 });
    expect(onSeek).toHaveBeenCalledWith(5000);
    fireEvent.click(screen.getByRole('slider'), { clientX: 500 });
    expect(onSeek).toHaveBeenCalledWith(10000);
  });

  it('renders segment blocks at proportional positions and active highlight', () => {
    const { rerender } = render(<SeekBar durationMs={10000} segments={[seg]} currentTimeMs={0} onSeek={vi.fn()} />);
    const block = screen.getByTestId('seg-1');
    expect(block.style.left).toBe('10%');
    expect(block.style.width).toBe('20%');
    expect(block.className).toContain('slate-500');

    rerender(<SeekBar durationMs={10000} segments={[seg]} currentTimeMs={2000} onSeek={vi.fn()} />);
    expect(screen.getByTestId('seg-1').className).toContain('blue-400');
    expect(screen.getByTestId('playhead').style.left).toBe('20%');
  });

  it('ignores clicks when duration is 0', () => {
    const onSeek = vi.fn();
    render(<SeekBar durationMs={0} segments={[]} currentTimeMs={0} onSeek={onSeek} />);
    fireEvent.click(screen.getByRole('slider'), { clientX: 100 });
    expect(onSeek).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/components/SeekBar.test.tsx`
Expected: FAIL — `./SeekBar` not found.

- [ ] **Step 3: Implement** — `ui/src/components/SeekBar.tsx`

```tsx
import React from 'react';
import type { SubtitleSegment } from '../types';

interface SeekBarProps {
  durationMs: number;
  segments: SubtitleSegment[];
  currentTimeMs: number;
  onSeek: (ms: number) => void;
}

export const SeekBar: React.FC<SeekBarProps> = ({ durationMs, segments, currentTimeMs, onSeek }) => {
  const pct = (ms: number) => Math.min(Math.max((ms / durationMs) * 100, 0), 100);

  const handleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (durationMs <= 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    if (rect.width <= 0) return;
    const ms = Math.round(((e.clientX - rect.left) / rect.width) * durationMs);
    onSeek(Math.min(Math.max(ms, 0), durationMs));
  };

  return (
    <div
      role="slider"
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={durationMs}
      aria-valuenow={currentTimeMs}
      onClick={handleClick}
      className="relative w-full h-2.5 bg-slate-700 rounded cursor-pointer overflow-hidden"
    >
      {durationMs > 0 &&
        segments.map((s) => (
          <div
            key={s.id}
            data-testid={`seg-${s.id}`}
            className={`absolute top-0 h-full ${
              currentTimeMs >= s.start_ms && currentTimeMs <= s.end_ms
                ? 'bg-blue-400/80'
                : 'bg-slate-500/60'
            }`}
            style={{
              left: `${pct(s.start_ms)}%`,
              width: `${Math.max(pct(s.end_ms) - pct(s.start_ms), 0)}%`,
              minWidth: '2px',
            }}
          />
        ))}
      {durationMs > 0 && (
        <div
          data-testid="playhead"
          className="absolute top-0 w-0.5 h-full bg-blue-300"
          style={{ left: `${pct(currentTimeMs)}%` }}
        />
      )}
    </div>
  );
};
```

- [ ] **Step 4: Integrate vào `ui/src/components/VideoPlayer.tsx`**

1. Thêm import sau dòng `import { SubtitleOverlay } from './SubtitleOverlay';`:

```tsx
import { SeekBar } from './SeekBar';
```

2. Đổi `handleSliderSeek` (dòng ~114–120) thành:

```tsx
  const handleSeek = (ms: number) => {
    if (videoRef.current) {
      videoRef.current.currentTime = ms / 1000;
      onTimeUpdate(ms);
    }
  };
```

3. Thay toàn bộ block seek hiện tại (dòng ~256–266):

```tsx
        {/* Seek timeline bar */}
        <div className="flex items-center gap-2">
          <input
            type="range"
            min={0}
            max={durationMs || 100}
            value={currentTimeMs}
            onChange={handleSliderSeek}
            disabled={!videoSrc}
            className="w-full h-1.5 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500"
          />
        </div>
```

bằng:

```tsx
        {/* Seek timeline bar with subtitle blocks */}
        <div className="flex items-center gap-2">
          <SeekBar
            durationMs={durationMs}
            segments={segments}
            currentTimeMs={currentTimeMs}
            onSeek={handleSeek}
          />
        </div>
```

- [ ] **Step 5: Run tests**

Run: `npm --prefix ui test -- src/components/SeekBar.test.tsx && npm --prefix ui run build`
Expected: 3/3 PASS; tsc + vite build OK (không còn tham chiếu `handleSliderSeek`).

- [ ] **Step 6: Commit**

```bash
git add ui/src/components/SeekBar.tsx ui/src/components/SeekBar.test.tsx ui/src/components/VideoPlayer.tsx
git commit -m "feat(ui): SeekBar with subtitle segment blocks replaces range input"
```

---

### Task 3: useFileDrop hook

**Files:**
- Create: `ui/src/hooks/useFileDrop.ts`
- Test: `ui/src/hooks/useFileDrop.test.tsx`

**Interfaces:**
- Consumes: `isTauri()` từ `../lib/ipc` (kiểm tra `window.__TAURI_INTERNALS__`/`__TAURI__`); `@tauri-apps/api/window` → `getCurrentWindow().onDragDropEvent`.
- Produces:
  - `useFileDrop(onDrop: (paths: string[]) => void): { isDragging: boolean }` — `onDrop` LUÔN được gọi khi drop xảy ra, kể cả `paths` rỗng (App quyết định toast).
  - `hasValidExtension(name: string): boolean` — export để test.
  - `VALID_EXTENSIONS = ['mp4','mkv','mov','avi','webm','m4v','mp3','wav','flac','m4a','ogg']`.

- [ ] **Step 1: Write the failing test** — `ui/src/hooks/useFileDrop.test.tsx`

```tsx
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import React from 'react';

// isTauri() returns true under Vitest (MODE === 'test'), so the hook's
// environment choice must be driven by mocking the module instead of
// touching window.__TAURI_INTERNALS__.
const { isTauriMock } = vi.hoisted(() => ({ isTauriMock: vi.fn() }));
vi.mock('../lib/ipc', () => ({ isTauri: () => isTauriMock() }));

let dragCb: ((e: { payload: { type: string; paths: string[] } }) => void) | null = null;
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: vi.fn((cb: (e: { payload: { type: string; paths: string[] } }) => void) => {
      dragCb = cb;
      return Promise.resolve(() => {});
    }),
  }),
}));

import { useFileDrop, hasValidExtension } from './useFileDrop';

beforeEach(() => {
  dragCb = null;
  isTauriMock.mockReturnValue(false);
  (window as unknown as Record<string, unknown>).URL.createObjectURL = vi.fn(() => 'blob:mock');
});

describe('hasValidExtension', () => {
  it('accepts media extensions, case-insensitive', () => {
    expect(hasValidExtension('/v/A.MP4')).toBe(true);
    expect(hasValidExtension('C:\\x\\b.wav')).toBe(true);
    expect(hasValidExtension('/v/notes.txt')).toBe(false);
    expect(hasValidExtension('noext')).toBe(false);
  });
});

describe('useFileDrop (Tauri path)', () => {
  it('tracks dragging and filters paths on drop', async () => {
    isTauriMock.mockReturnValue(true);
    const onDrop = vi.fn();
    const { result } = renderHook(() => useFileDrop(onDrop));
    await act(async () => {}); // flush onDragDropEvent promise

    act(() => dragCb!({ payload: { type: 'over', paths: [] } }));
    expect(result.current.isDragging).toBe(true);

    act(() => dragCb!({ payload: { type: 'drop', paths: ['/v/a.mp4', '/v/b.txt', '/v/c.mkv'] } }));
    expect(result.current.isDragging).toBe(false);
    expect(onDrop).toHaveBeenCalledWith(['/v/a.mp4', '/v/c.mkv']);

    act(() => dragCb!({ payload: { type: 'over', paths: [] } }));
    act(() => dragCb!({ payload: { type: 'leave', paths: [] } }));
    expect(result.current.isDragging).toBe(false);
  });
});

describe('useFileDrop (browser path)', () => {
  it('uses document listeners and object URLs', () => {
    const onDrop = vi.fn();
    const { result } = renderHook(() => useFileDrop(onDrop));

    act(() => {
      document.dispatchEvent(new Event('dragover'));
    });
    expect(result.current.isDragging).toBe(true);

    const good = new File(['x'], 'a.mp4');
    const bad = new File(['x'], 'b.txt');
    act(() => {
      const e = new Event('drop', { bubbles: true }) as Event & { dataTransfer: { files: File[] } };
      e.dataTransfer = { files: [good, bad] };
      document.dispatchEvent(e);
    });
    expect(result.current.isDragging).toBe(false);
    expect(onDrop).toHaveBeenCalledWith(['blob:mock']);
  });
});
```

Lưu ý test: `isTauri()` trong `../lib/ipc` trả `true` khi `import.meta.env.MODE === 'test'` — nghĩa là dưới Vitest hook luôn thấy "đang trong Tauri". Vì vậy test mock `../lib/ipc` để điều khiển nhánh Tauri/browser; App.test đã mock `isTauri: () => false` nên case overlay ở Task 5 đi đúng đường document listeners.

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/hooks/useFileDrop.test.tsx`
Expected: FAIL — `./useFileDrop` not found.

- [ ] **Step 3: Implement** — `ui/src/hooks/useFileDrop.ts`

```ts
import { useEffect, useRef, useState } from 'react';
import { getCurrentWindow, type UnlistenFn } from '@tauri-apps/api/window';
import { isTauri } from '../lib/ipc';

export const VALID_EXTENSIONS = [
  'mp4', 'mkv', 'mov', 'avi', 'webm', 'm4v',
  'mp3', 'wav', 'flac', 'm4a', 'ogg',
];

export function hasValidExtension(name: string): boolean {
  const idx = name.lastIndexOf('.');
  if (idx < 0) return false;
  return VALID_EXTENSIONS.includes(name.slice(idx + 1).toLowerCase());
}

export function useFileDrop(onDrop: (paths: string[]) => void): { isDragging: boolean } {
  const [isDragging, setIsDragging] = useState(false);
  const onDropRef = useRef(onDrop);
  onDropRef.current = onDrop;

  useEffect(() => {
    if (!isTauri()) {
      const onDragOver = (e: Event) => {
        e.preventDefault();
        setIsDragging(true);
      };
      const onDragLeave = (e: Event) => {
        if (!(e as DragEvent).relatedTarget) setIsDragging(false);
      };
      const onDropEvent = (e: Event) => {
        e.preventDefault();
        setIsDragging(false);
        const files = Array.from((e as DragEvent).dataTransfer?.files ?? []).filter((f) =>
          hasValidExtension(f.name)
        );
        onDropRef.current(files.map((f) => URL.createObjectURL(f)));
      };
      document.addEventListener('dragover', onDragOver);
      document.addEventListener('dragleave', onDragLeave);
      document.addEventListener('drop', onDropEvent);
      return () => {
        document.removeEventListener('dragover', onDragOver);
        document.removeEventListener('dragleave', onDragLeave);
        document.removeEventListener('drop', onDropEvent);
      };
    }

    let unlisten: UnlistenFn | undefined;
    let cancelled = false;
    getCurrentWindow()
      .onDragDropEvent((e) => {
        const type = e.payload.type;
        if (type === 'enter' || type === 'over') setIsDragging(true);
        else if (type === 'leave') setIsDragging(false);
        else if (type === 'drop') {
          setIsDragging(false);
          onDropRef.current(e.payload.paths.filter(hasValidExtension));
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => console.warn('onDragDropEvent failed:', err));
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  return { isDragging };
}
```

- [ ] **Step 4: Run tests**

Run: `npm --prefix ui test -- src/hooks/useFileDrop.test.tsx`
Expected: 3/3 PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/hooks/useFileDrop.ts ui/src/hooks/useFileDrop.test.tsx
git commit -m "feat(ui): useFileDrop hook — Tauri drag-drop with browser fallback"
```

---

### Task 4: useToasts hook + Toasts component

**Files:**
- Create: `ui/src/hooks/useToasts.ts`
- Create: `ui/src/components/Toasts.tsx`
- Test: `ui/src/components/Toasts.test.tsx` (gộp test hook + component)

**Interfaces:**
- Produces:
  - `interface Toast { id: number; text: string; kind: 'info' | 'success' | 'error' }` (export từ `Toasts.tsx`).
  - `useToasts(): { toasts: Toast[]; pushToast: (text: string, kind?: Toast['kind']) => void; dismissToast: (id: number) => void }`.
  - `Toasts` — props `{ toasts: Toast[]; onDismiss: (id: number) => void }`.

- [ ] **Step 1: Write the failing test** — `ui/src/components/Toasts.test.tsx`

```tsx
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, renderHook, act } from '@testing-library/react';
import React from 'react';
import { Toasts, type Toast } from './Toasts';
import { useToasts } from '../hooks/useToasts';

describe('Toasts component', () => {
  it('renders toasts and dismiss button calls onDismiss', () => {
    const onDismiss = vi.fn();
    const toasts: Toast[] = [
      { id: 1, text: 'Lỗi mạng', kind: 'error' },
      { id: 2, text: 'Xong', kind: 'success' },
    ];
    render(<Toasts toasts={toasts} onDismiss={onDismiss} />);
    expect(screen.getByText('Lỗi mạng')).toBeDefined();
    expect(screen.getByText('Xong')).toBeDefined();
    fireEvent.click(screen.getAllByLabelText('Đóng thông báo')[0]);
    expect(onDismiss).toHaveBeenCalledWith(1);
  });
});

describe('useToasts', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('pushes, auto-dismisses after 4s, caps at 4', () => {
    const { result } = renderHook(() => useToasts());
    act(() => result.current.pushToast('một', 'info'));
    expect(result.current.toasts).toHaveLength(1);
    expect(result.current.toasts[0].text).toBe('một');

    act(() => {
      for (let i = 0; i < 5; i++) result.current.pushToast(`t${i}`);
    });
    expect(result.current.toasts).toHaveLength(4);

    act(() => vi.advanceTimersByTime(4100));
    expect(result.current.toasts).toHaveLength(0);
  });

  it('dismissToast removes immediately and clears timer', () => {
    const { result } = renderHook(() => useToasts());
    act(() => result.current.pushToast('x'));
    const id = result.current.toasts[0].id;
    act(() => result.current.dismissToast(id));
    expect(result.current.toasts).toHaveLength(0);
    act(() => vi.advanceTimersByTime(10000));
    expect(result.current.toasts).toHaveLength(0);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/components/Toasts.test.tsx`
Expected: FAIL — `./Toasts` not found.

- [ ] **Step 3: Implement** — `ui/src/components/Toasts.tsx`

```tsx
import React from 'react';
import { X, CheckCircle2, Info, AlertTriangle } from 'lucide-react';

export interface Toast {
  id: number;
  text: string;
  kind: 'info' | 'success' | 'error';
}

const KIND_STYLE: Record<Toast['kind'], string> = {
  info: 'bg-slate-800 border-slate-600 text-slate-200',
  success: 'bg-emerald-900/90 border-emerald-700 text-emerald-200',
  error: 'bg-rose-900/90 border-rose-700 text-rose-200',
};

const KIND_ICON: Record<Toast['kind'], React.ReactNode> = {
  info: <Info className="w-3.5 h-3.5 shrink-0" />,
  success: <CheckCircle2 className="w-3.5 h-3.5 shrink-0" />,
  error: <AlertTriangle className="w-3.5 h-3.5 shrink-0" />,
};

export const Toasts: React.FC<{ toasts: Toast[]; onDismiss: (id: number) => void }> = ({
  toasts,
  onDismiss,
}) => (
  <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
    {toasts.map((t) => (
      <div
        key={t.id}
        className={`flex items-start gap-2 rounded-md border px-3 py-2 text-xs shadow-lg ${KIND_STYLE[t.kind]}`}
      >
        {KIND_ICON[t.kind]}
        <span className="flex-1">{t.text}</span>
        <button
          aria-label="Đóng thông báo"
          onClick={() => onDismiss(t.id)}
          className="shrink-0 opacity-70 hover:opacity-100"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
    ))}
  </div>
);
```

`ui/src/hooks/useToasts.ts`:

```ts
import { useCallback, useRef, useState } from 'react';
import type { Toast } from '../components/Toasts';

const MAX_TOASTS = 4;
const AUTO_DISMISS_MS = 4000;

export function useToasts() {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const idRef = useRef(0);
  const timers = useRef(new Map<number, ReturnType<typeof setTimeout>>());

  const dismissToast = useCallback((id: number) => {
    const t = timers.current.get(id);
    if (t) {
      clearTimeout(t);
      timers.current.delete(id);
    }
    setToasts((prev) => prev.filter((x) => x.id !== id));
  }, []);

  const pushToast = useCallback(
    (text: string, kind: Toast['kind'] = 'info') => {
      const id = ++idRef.current;
      setToasts((prev) => [...prev.slice(-(MAX_TOASTS - 1)), { id, text, kind }]);
      timers.current.set(id, setTimeout(() => dismissToast(id), AUTO_DISMISS_MS));
    },
    [dismissToast]
  );

  return { toasts, pushToast, dismissToast };
}
```

- [ ] **Step 4: Run tests**

Run: `npm --prefix ui test -- src/components/Toasts.test.tsx`
Expected: 3/3 PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/hooks/useToasts.ts ui/src/components/Toasts.tsx ui/src/components/Toasts.test.tsx
git commit -m "feat(ui): toast notifications with auto-dismiss"
```

---

### Task 5: App wiring — SplitPane layout, drop overlay, toasts, JobList empty state

**Files:**
- Modify: `ui/src/App.tsx`, `ui/src/App.test.tsx`, `ui/src/components/JobList.tsx`

**Interfaces:**
- Consumes: Task 1 (`SplitPane`), Task 3 (`useFileDrop`), Task 4 (`useToasts`, `Toasts`).

- [ ] **Step 1: Add the failing test** — append vào `describe` trong `ui/src/App.test.tsx`:

```tsx
  it('shows drop overlay on dragover', async () => {
    render(<App />);
    await waitFor(() => screen.getByDisplayValue('English (EN)'));
    fireEvent.dragOver(document);
    expect(screen.getByText('Thả để thêm vào hàng đợi')).toBeDefined();
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test -- src/App.test.tsx`
Expected: FAIL — không có overlay.

- [ ] **Step 3: Implement `ui/src/App.tsx`**

1. Imports — thêm `FileVideo` vào lucide (hiện chưa import lucide trong App; thêm dòng mới), thêm components/hooks:

```tsx
import { FileVideo } from 'lucide-react';
import { SplitPane } from './components/SplitPane';
import { Toasts } from './components/Toasts';
import { useFileDrop } from './hooks/useFileDrop';
import { useToasts } from './hooks/useToasts';
```

2. Sau dòng `const settingsRef = useRef<SettingsView | null>(null); settingsRef.current = settingsView;` thêm:

```tsx
  const { toasts, pushToast, dismissToast } = useToasts();
```

3. Sau hàm `addJobs` thêm:

```tsx
  const handleFilesDropped = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) {
        pushToast('Không có file video/audio hợp lệ', 'error');
        return;
      }
      addJobs(paths);
    },
    [addJobs, pushToast]
  );
  const { isDragging } = useFileDrop(handleFilesDropped);
```

4. Trong `runJob`, sửa hai chỗ:

- Sau `updateJob(job.id, { status: 'done', segments: result });` thêm:

```tsx
      pushToast(`Hoàn tất ${job.videoPath.split(/[/\\]/).pop()}`, 'success');
```

- Trong `catch`, sau `setLogs(...)` thêm:

```tsx
      pushToast(msg, 'error');
```

5. Trong `handleSelectProfile` và `handleSelectSkill`, trong `catch` sau `setLogs(...)` thêm `pushToast(String(err), 'error');`.

6. Trong `useEffect` load settings, sau `if (s.load_warning) setLogs(...)` thêm:

```tsx
        if (s.load_warning) pushToast(s.load_warning, 'info');
```

7. Đổi layout `<main>` — thay:

```tsx
      <main className="flex-1 min-h-0 grid grid-cols-1 md:grid-cols-2 gap-3 p-3 overflow-hidden">
        {/* Left Column: Video Player + Job List */}
        <section className="h-full min-h-0 flex flex-col gap-2">
          <div className="flex-1 min-h-0">
            <VideoPlayer ... />
          </div>

          {/* Batch job / output list */}
          <div className="h-44 shrink-0 bg-slate-900 border border-slate-800 rounded-lg overflow-hidden flex flex-col">
            ...JobList...
          </div>
        </section>

        {/* Right Column: Subtitle Editor */}
        <section className="h-full min-h-0">
          <SubtitleEditor ... />
        </section>
      </main>
```

bằng:

```tsx
      <main className="flex-1 min-h-0 p-3 overflow-hidden">
        <SplitPane direction="horizontal" initialRatio={0.5} minA={420} minB={360} storageKey="main">
          {/* Left Column: Video Player + Job List */}
          <section className="h-full min-h-0 pr-1.5">
            <SplitPane direction="vertical" initialRatio={0.7} minA={240} minB={120} storageKey="video-list">
              <div className="h-full min-h-0 pb-1">
                <VideoPlayer ...giữ nguyên props... />
              </div>

              {/* Batch job / output list */}
              <div className="h-full min-h-0 pt-1 bg-slate-900 border border-slate-800 rounded-lg overflow-hidden flex flex-col">
                ...JobList giữ nguyên...
              </div>
            </SplitPane>
          </section>

          {/* Right Column: Subtitle Editor */}
          <section className="h-full min-h-0 pl-1.5">
            <SubtitleEditor ...giữ nguyên props... />
          </section>
        </SplitPane>
      </main>
```

(Giữ nguyên toàn bộ JSX của `VideoPlayer`, `JobList`, `SubtitleEditor` — chỉ đổi wrapper.)

8. Trước `</div>` đóng của root (sau `{/* 5. Settings Modal */}`), thêm:

```tsx
      {/* 6. Drag overlay + Toasts */}
      {isDragging && (
        <div className="fixed inset-0 z-40 bg-blue-950/60 border-2 border-dashed border-blue-400 flex items-center justify-center pointer-events-none">
          <div className="flex flex-col items-center gap-2 text-blue-200">
            <FileVideo className="w-10 h-10" />
            <span className="text-sm font-medium">Thả để thêm vào hàng đợi</span>
          </div>
        </div>
      )}
      <Toasts toasts={toasts} onDismiss={dismissToast} />
```

9. `ui/src/components/JobList.tsx` — đổi empty state (dòng ~74–79):

```tsx
    return (
      <div className="h-full flex flex-col items-center justify-center gap-2 text-slate-600 text-[11px]">
        <FileVideo className="w-8 h-8 opacity-30" />
        <span className="italic">Chưa có file nào trong danh sách. Kéo thả hoặc chọn video để bắt đầu.</span>
      </div>
    );
```

(`FileVideo` đã được import sẵn trong JobList.)

- [ ] **Step 4: Run tests + build**

Run: `npm --prefix ui test && npm --prefix ui run build`
Expected: tất cả test PASS; tsc + vite build OK.

- [ ] **Step 5: Commit**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/JobList.tsx
git commit -m "feat(ui): resizable split layout, drag-drop overlay, toast wiring"
```

---

### Task 6: Full verification

**Files:** none (fix-ups only).

- [ ] **Step 1:** `npm --prefix ui test` — expected all pass (số test tăng ~14 so với trước).
- [ ] **Step 2:** `npm --prefix ui run build` — expected `✓ built`.
- [ ] **Step 3:** `cd src-tauri && npx tauri build --bundles deb` — expected `Finished 1 bundle` (xác nhận release path vẫn nguyên sau khi đổi layout).
- [ ] **Step 4: Manual smoke (user)** — `npx tauri dev`: kéo một file `.mp4` thả vào cửa sổ → overlay hiện, thả → vào hàng đợi; kéo divider giữa hai cột và giữa video/danh sách → resize được, restart vẫn giữ tỉ lệ; chạy job xong → toast "Hoàn tất ..."; click SeekBar → video tua, block phụ đề đúng vị trí.
- [ ] **Step 5:** Fix-ups nếu có → `git add -A && git commit -m "chore: fix-ups from UI improvements verification"`.
