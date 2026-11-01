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
