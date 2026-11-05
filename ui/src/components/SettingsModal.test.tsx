import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import { SettingsModal, type SettingsApi } from './SettingsModal';
import type { SettingsView, GeneralSettings } from '../types';

const general: GeneralSettings = {
  whisper_model: 'base',
  target_lang: 'vi',
  bilingual: true,
  style_preset: 'youtube_yellow',
  use_cloud_stt: false,
  prefer_gpu: true,
  dub_enabled: false,
  tts_engine: 'local',
  tts_voice: null,
  tts_model: null,
  tts_base_url: null,
  dub_orig_volume: 0.15,
  render_speed: 'balanced',
  translate_concurrency: 4,
      auto_export: true,
      pad_subs: false,
};

const view = (over: Partial<SettingsView> = {}): SettingsView => ({
  profiles: [
    { id: 'o', name: 'Ollama local', kind: 'ollama', base_url: null, model: 'qwen2.5:7b', has_key: false },
    { id: 'd', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'deepseek-chat', has_key: true },
  ],
  default_profile_id: 'o',
  general,
  presets: [],
  skills: [],
  default_skill_id: null,
  stt_has_key: false,
  secrets_backend: 'keyring',
  load_warning: null,
  ...over,
});

let api: SettingsApi;
beforeEach(() => {
  api = {
    saveProfile: vi.fn().mockResolvedValue(view()),
    deleteProfile: vi.fn().mockResolvedValue(view()),
    setDefaultProfile: vi.fn().mockResolvedValue(view()),
    testProfile: vi.fn().mockResolvedValue({ ok: true, latency_ms: 123, error: null }),
    listProfileModels: vi.fn().mockResolvedValue(['deepseek-chat', 'deepseek-reasoner']),
    setSttKey: vi.fn().mockResolvedValue(view()),
  };
});

const renderModal = (settings = view(), onSettingsChange = vi.fn()) =>
  render(
    <SettingsModal
      isOpen
      onClose={vi.fn()}
      settings={settings}
      onSettingsChange={onSettingsChange}
      general={general}
      onGeneralChange={vi.fn()}
      whisperModels={[{ name: 'base', downloaded: true, size_mb: 142 }]}
      api={api}
    />
  );

// Profile names also appear as <option> labels in the Kind select, so target the list buttons.
const pickProfile = (name: string) =>
  fireEvent.click(screen.getByRole('button', { name: new RegExp(name) }));

describe('SettingsModal', () => {
  it('lists profiles with default star and no keychain banner', () => {
    renderModal();
    expect(screen.getByRole('button', { name: /★ Ollama local/ })).toBeDefined();
    expect(screen.getByRole('button', { name: /DeepSeek/ })).toBeDefined();
    expect(screen.queryByText(/Không tìm thấy keychain/)).toBeNull();
  });

  it('shows keychain banner for file backend', () => {
    renderModal(view({ secrets_backend: 'file' }));
    expect(screen.getByText(/Không tìm thấy keychain hệ thống/)).toBeDefined();
  });

  it('saving with empty key keeps stored key (null)', async () => {
    const onChange = vi.fn();
    renderModal(view(), onChange);
    pickProfile('DeepSeek');
    expect(screen.getByPlaceholderText('••• đã lưu')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Lưu' }));
    await waitFor(() =>
      expect(api.saveProfile).toHaveBeenCalledWith(
        { id: 'd', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'deepseek-chat' },
        null
      )
    );
    expect(onChange).toHaveBeenCalled();
  });

  it('typed key is sent; Xóa key sends empty string', async () => {
    renderModal();
    pickProfile('DeepSeek');
    fireEvent.change(screen.getByLabelText('API key'), { target: { value: 'sk-new' } });
    fireEvent.click(screen.getByRole('button', { name: 'Lưu' }));
    await waitFor(() => expect(api.saveProfile).toHaveBeenLastCalledWith(expect.objectContaining({ id: 'd' }), 'sk-new'));
    fireEvent.click(screen.getByRole('button', { name: 'Xóa key' }));
    await waitFor(() => expect(api.saveProfile).toHaveBeenLastCalledWith(expect.objectContaining({ id: 'd' }), ''));
  });

  it('test connection shows success and failure', async () => {
    renderModal();
    pickProfile('DeepSeek');
    fireEvent.click(screen.getByRole('button', { name: 'Test kết nối' }));
    expect(await screen.findByText('✅ Kết nối OK (123 ms)')).toBeDefined();
    (api.testProfile as any).mockResolvedValueOnce({ ok: false, latency_ms: 5, error: 'API key không hợp lệ' });
    fireEvent.click(screen.getByRole('button', { name: 'Test kết nối' }));
    expect(await screen.findByText('❌ API key không hợp lệ')).toBeDefined();
  });

  it('new profile is saved with empty id; delete asks confirmation', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    renderModal();
    fireEvent.click(screen.getByRole('button', { name: '+ Thêm' }));
    fireEvent.change(screen.getByLabelText('Tên'), { target: { value: 'Claude' } });
    fireEvent.change(screen.getByLabelText('Loại'), { target: { value: 'anthropic' } });
    fireEvent.change(screen.getByLabelText('Model'), { target: { value: 'claude-x' } });
    fireEvent.click(screen.getByRole('button', { name: 'Lưu' }));
    await waitFor(() =>
      expect(api.saveProfile).toHaveBeenCalledWith(
        { id: '', name: 'Claude', kind: 'anthropic', base_url: null, model: 'claude-x' },
        null
      )
    );

    pickProfile('DeepSeek');
    fireEvent.click(screen.getByRole('button', { name: 'Xóa' }));
    await waitFor(() => expect(api.deleteProfile).toHaveBeenCalledWith('d'));
  });

  it('model suggestions are fetched for the selected profile', async () => {
    renderModal();
    pickProfile('DeepSeek');
    await waitFor(() => expect(api.listProfileModels).toHaveBeenCalled());
    await waitFor(() => {
      const select = screen.getByLabelText('Model') as HTMLSelectElement;
      expect([...select.options].some((o) => o.value === 'deepseek-reasoner')).toBe(true);
    });
  });

  it('has a Skills tab', () => {
    renderModal();
    fireEvent.click(screen.getByRole('button', { name: 'Skills' }));
    expect(screen.getByRole('button', { name: '+ Thêm skill' })).toBeDefined();
  });
});
