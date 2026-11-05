import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import type { SettingsView } from './types';

// vi.mock is hoisted above imports, so shared fixtures must come from vi.hoisted.
const { loaded, saveGeneral, startPipeline } = vi.hoisted(() => {
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
      prefer_gpu: true,
      dub_enabled: false,
      tts_engine: 'local' as const,
      tts_voice: null,
      tts_model: null,
      tts_base_url: null,
      dub_orig_volume: 0.15,
      render_speed: 'balanced' as const,
      translate_concurrency: 4,
      auto_export: true,
      pad_subs: false,
    },
    presets: [{ id: 'builtin:math', name: 'Toán học', description: '', system_prompt: 'p', glossary: [] }],
    skills: [],
    default_skill_id: 'builtin:math',
    stt_has_key: false,
    secrets_backend: 'keyring',
    load_warning: 'File cài đặt bị hỏng',
  };
  return {
    loaded,
    saveGeneral: vi.fn().mockResolvedValue(loaded),
    startPipeline: vi.fn().mockResolvedValue([]),
  };
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
    startPipeline: (...args: unknown[]) => startPipeline(...args),
  };
});

import App from './App';

beforeEach(() => {
  saveGeneral.mockClear();
  startPipeline.mockClear();
});

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

  it('shows drop overlay on dragover', async () => {
    render(<App />);
    await waitFor(() => screen.getByDisplayValue('English (EN)'));
    fireEvent.dragOver(document);
    expect(screen.getByText('Thả để thêm vào hàng đợi')).toBeDefined();
  });

  it('asks before re-adding a file already in the queue', async () => {
    (URL as any).createObjectURL = vi.fn(() => 'blob:a.mp4');
    render(<App />);
    await waitFor(() => screen.getByDisplayValue('English (EN)'));

    const file = new File(['x'], 'a.mp4', { type: 'video/mp4' });
    const drop = () =>
      fireEvent.drop(document, { dataTransfer: { files: [file] } });

    drop();
    await waitFor(() => expect(screen.getByText('Danh sách xử lý (1)')).toBeTruthy());

    drop();
    expect(await screen.findByText('1 file đã có trong hàng đợi')).toBeTruthy();

    // skip -> still one job; dialog closes
    fireEvent.click(screen.getByRole('button', { name: /bỏ qua file trùng/i }));
    await waitFor(() => expect(screen.queryByText('1 file đã có trong hàng đợi')).toBeNull());
    expect(screen.getByText('Danh sách xử lý (1)')).toBeTruthy();

    // add again anyway -> two jobs
    drop();
    fireEvent.click(await screen.findByRole('button', { name: /vẫn thêm/i }));
    await waitFor(() => expect(screen.getByText('Danh sách xử lý (2)')).toBeTruthy());
  });

  it('opens config modal when clicking start button and runs pipeline after confirmation', async () => {
    (URL as any).createObjectURL = vi.fn(() => 'blob:test-video.mp4');
    render(<App />);
    await waitFor(() => screen.getByDisplayValue('English (EN)'));

    const file = new File(['test-video'], 'test-video.mp4', { type: 'video/mp4' });
    fireEvent.drop(document, { dataTransfer: { files: [file] } });
    await waitFor(() => expect(screen.getByText('Danh sách xử lý (1)')).toBeTruthy());

    // Click Start button on HeaderBar
    const startBtn = screen.getByRole('button', { name: /Bắt Đầu/i });
    expect((startBtn as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(startBtn);

    // RunConfigModal should appear
    expect(await screen.findByText('Xác nhận cấu hình trước khi chạy')).toBeTruthy();
    expect(screen.getByText('test-video.mp4')).toBeTruthy();

    // Confirm run in modal
    const confirmBtn = screen.getByRole('button', { name: /Bắt đầu chạy/i });
    fireEvent.click(confirmBtn);

    // Modal should close and pipeline starts
    await waitFor(() => expect(screen.queryByText('Xác nhận cấu hình trước khi chạy')).toBeNull());
    await waitFor(() => expect(startPipeline).toHaveBeenCalledTimes(1));
  });
});
