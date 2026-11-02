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
