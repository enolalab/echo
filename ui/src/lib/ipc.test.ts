import { describe, it, expect, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { getSystemInfo, listenPipelineProgress, getSettings, saveProfile, testProfile, listProfileModels, saveGeneral, saveSkill, setDefaultSkill, trySkill, duplicateSkill } from './ipc';
import { defaultBaseUrl, requiresBaseUrl, requiresApiKey, emptyProfile, KIND_LABELS } from './providers';

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

describe('settings IPC', () => {
  it('passes camelCase apiKey and null to keep the stored key', async () => {
    const profile = { id: 'p1', name: 'DS', kind: 'deepseek' as const, base_url: null, model: 'deepseek-chat' };
    await saveProfile(profile, null);
    expect(invoke).toHaveBeenCalledWith('save_profile', { profile, apiKey: null });
    await testProfile(profile, 'sk');
    expect(invoke).toHaveBeenCalledWith('test_profile', { profile, apiKey: 'sk' });
    await listProfileModels(profile, null);
    expect(invoke).toHaveBeenCalledWith('list_profile_models', { profile, apiKey: null });
  });

  it('calls get_settings and save_general', async () => {
    await getSettings();
    expect(invoke).toHaveBeenCalledWith('get_settings');
    const general = { whisper_model: 'base', target_lang: 'vi', bilingual: true, style_preset: 'youtube_yellow' as const, use_cloud_stt: false, prefer_gpu: true, dub_enabled: false, tts_engine: 'local' as const, dub_orig_volume: 0.15, render_speed: 'balanced' as const, translate_concurrency: 4, auto_export: true };
    await saveGeneral(general);
    expect(invoke).toHaveBeenCalledWith('save_general', { general });
  });
});

describe('provider helpers', () => {
  it('knows defaults and requirements', () => {
    expect(defaultBaseUrl('deepseek')).toBe('https://api.deepseek.com/v1');
    expect(defaultBaseUrl('openai_compatible')).toBeNull();
    expect(requiresBaseUrl('anthropic_compatible')).toBe(true);
    expect(requiresApiKey('ollama')).toBe(false);
    expect(KIND_LABELS.anthropic).toBe('Anthropic');
    expect(emptyProfile()).toEqual({ id: '', name: '', kind: 'openai', base_url: null, model: '' });
  });
});

describe('skill IPC', () => {
  it('uses the agreed command names and camelCase args', async () => {
    const skill = { id: '', name: 'X', description: '', system_prompt: 'p', glossary: [] };
    await saveSkill(skill);
    expect(invoke).toHaveBeenCalledWith('save_skill', { skill });
    await setDefaultSkill(null);
    expect(invoke).toHaveBeenCalledWith('set_default_skill', { id: null });
    await duplicateSkill('builtin:math');
    expect(invoke).toHaveBeenCalledWith('duplicate_skill', { id: 'builtin:math' });
    await trySkill(skill, null, 'Hello', 'auto', 'vi');
    expect(invoke).toHaveBeenCalledWith('try_skill', {
      skill,
      profileId: null,
      sample: 'Hello',
      sourceLang: 'auto',
      targetLang: 'vi',
    });
  });
});
