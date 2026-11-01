import { describe, it, expect, vi } from 'vitest';
import { getSystemInfo } from './ipc';

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

describe('IPC Client', () => {
  it('fetches system info correctly', async () => {
    const info = await getSystemInfo();
    expect(info.ollama_running).toBe(true);
    expect(info.ollama_models).toContain('qwen2.5:7b');
    expect(info.whisper_models[0].name).toBe('base');
  });
});
