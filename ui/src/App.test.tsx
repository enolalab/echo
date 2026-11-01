import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import type { SettingsView } from './types';

// vi.mock is hoisted above imports, so shared fixtures must come from vi.hoisted.
const { loaded, saveGeneral } = vi.hoisted(() => {
  const loaded: SettingsView = {
    profiles: [
      { id: 'd', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'deepseek-chat', has_key: true },
    ],
    default_profile_id: 'd',
    general: {
      whisper_model: 'small',
      target_lang: 'en',
      bilingual: true,
      style_preset: 'netflix_modern',
      use_cloud_stt: false,
    },
    presets: [{ id: 'builtin:math', name: 'Toán học', description: '', system_prompt: 'p', glossary: [] }],
    skills: [],
    default_skill_id: 'builtin:math',
    stt_has_key: false,
    secrets_backend: 'keyring',
    load_warning: 'File cài đặt bị hỏng',
  };
  return { loaded, saveGeneral: vi.fn().mockResolvedValue(loaded) };
});

vi.mock('./lib/ipc', async (importOriginal) => {
  const mod = await importOriginal<typeof import('./lib/ipc')>();
  return {
    ...mod,
    isTauri: () => false,
    getSystemInfo: vi.fn().mockResolvedValue({
      has_cuda: false,
      ollama_running: false,
      ollama_models: [],
      whisper_models: [
        { name: 'base', downloaded: true, size_mb: 142 },
        { name: 'small', downloaded: true, size_mb: 466 },
      ],
    }),
    getOutputDir: vi.fn().mockResolvedValue('/out'),
    listenPipelineProgress: vi.fn().mockResolvedValue(() => {}),
    getSettings: vi.fn().mockResolvedValue(loaded),
    saveGeneral: (...args: unknown[]) => saveGeneral(...args),
    setDefaultProfile: vi.fn().mockResolvedValue(loaded),
    setDefaultSkill: vi.fn().mockResolvedValue(loaded),
  };
});

import App from './App';

beforeEach(() => saveGeneral.mockClear());

describe('App settings persistence', () => {
  it('loads general settings and default profile without re-saving', async () => {
    render(<App />);
    await waitFor(() =>
      expect((screen.getByDisplayValue('English (EN)') as HTMLSelectElement).value).toBe('en')
    );
    expect((screen.getByLabelText('Provider profile') as HTMLSelectElement).value).toBe('d');
    expect(saveGeneral).not.toHaveBeenCalled();
  });

  it('debounces save_general after header change', async () => {
    render(<App />);
    await waitFor(() => screen.getByDisplayValue('English (EN)'));
    fireEvent.click(screen.getByRole('button', { name: /Song ngữ/ }));
    expect(saveGeneral).not.toHaveBeenCalled();
    await waitFor(
      () => expect(saveGeneral).toHaveBeenCalledWith(expect.objectContaining({ bilingual: false, target_lang: 'en' })),
      { timeout: 2000 }
    );
    expect(saveGeneral).toHaveBeenCalledTimes(1);
  });

  it('header shows default skill from settings', async () => {
    render(<App />);
    await waitFor(() =>
      expect((screen.getByLabelText('Skill') as HTMLSelectElement).value).toBe('builtin:math')
    );
  });
});
