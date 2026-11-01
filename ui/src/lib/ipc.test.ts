import { describe, it, expect, vi } from 'vitest';
import { getSystemInfo, listenPipelineProgress } from './ipc';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockImplementation((cmd: string) => {
    if (cmd === 'get_system_info') {
      return Promise.resolve({
        has_cuda: false,
        ollama_running: true,
        ollama_models: ['qwen2.5:7b'],
        whisper_models: [
          { name: 'base', downloaded: true, size_mb: 142 }
        ],
      });
    }
    return Promise.resolve();
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation((_event: string, cb: (e: any) => void) => {
    cb({
      payload: {
        stage: 'translate',
        message: 'Đang dịch...',
        percentage: 75.0,
        elapsed_secs: 15,
      },
    });
    return Promise.resolve(() => {});
  }),
}));

describe('IPC Client', () => {
  it('fetches system info correctly', async () => {
    const info = await getSystemInfo();
    expect(info.ollama_running).toBe(true);
    expect(info.ollama_models).toContain('qwen2.5:7b');
    expect(info.whisper_models[0].name).toBe('base');
  });

  it('listens to pipeline progress events', async () => {
    let received: any = null;
    const unlisten = await listenPipelineProgress((event) => {
      received = event;
    });
    expect(received).not.toBeNull();
    expect(received.percentage).toBe(75.0);
    unlisten();
  });
});
