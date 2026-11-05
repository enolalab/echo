import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { RunConfigModal } from './RunConfigModal';
import type { GeneralSettings, ProfileView, Skill, SystemInfo, BatchJob } from '../types';

const mockGeneral: GeneralSettings = {
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

const mockProfiles: ProfileView[] = [
  { id: 'p1', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'deepseek-chat', has_key: true },
  { id: 'p2', name: 'Ollama', kind: 'ollama', base_url: null, model: 'qwen2.5:7b', has_key: false },
];

const mockPresets: Skill[] = [
  { id: 's1', name: 'Công nghệ', description: '', system_prompt: '', glossary: [] },
];

const mockSystemInfo: SystemInfo = {
  has_cuda: true,
  cuda_ready: true,
  cuda_error: null,
  ffmpeg_ready: true,
  ollama_running: true,
  ollama_models: ['qwen2.5:7b'],
  whisper_models: [
    { name: 'base', downloaded: true, size_mb: 142 },
    { name: 'large-v3', downloaded: false, size_mb: 3000 },
  ],
};

const mockQueuedJobs: BatchJob[] = [
  {
    id: 'j1',
    videoPath: '/videos/sample1.mp4',
    status: 'queued',
    progress: null,
    segments: [],
    outputVideoPath: null,
    error: null,
  },
  {
    id: 'j2',
    videoPath: '/videos/sample2.mkv',
    status: 'queued',
    progress: null,
    segments: [],
    outputVideoPath: null,
    error: null,
  },
];

describe('RunConfigModal', () => {
  it('does not render when isOpen is false', () => {
    const { container } = render(
      <RunConfigModal
        isOpen={false}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
        queuedJobs={[]}
        general={mockGeneral}
        systemInfo={mockSystemInfo}
        profiles={mockProfiles}
        defaultProfileId="p1"
        presets={mockPresets}
        userSkills={[]}
        defaultSkillId={null}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders modal with queued files and current configuration when isOpen is true', () => {
    render(
      <RunConfigModal
        isOpen={true}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
        queuedJobs={mockQueuedJobs}
        general={mockGeneral}
        systemInfo={mockSystemInfo}
        profiles={mockProfiles}
        defaultProfileId="p1"
        presets={mockPresets}
        userSkills={[]}
        defaultSkillId="s1"
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );

    expect(screen.getByText('Xác nhận cấu hình trước khi chạy')).toBeDefined();
    expect(screen.getByText(/Danh sách tệp xử lý \(2\)/)).toBeDefined();
    expect(screen.getByText('sample1.mp4')).toBeDefined();
    expect(screen.getByText('sample2.mkv')).toBeDefined();

    // Verify fields
    const whisperSelect = screen.getByLabelText('Mô hình Whisper') as HTMLSelectElement;
    expect(whisperSelect.value).toBe('base');

    const profileSelect = screen.getByLabelText('AI Dịch thuật') as HTMLSelectElement;
    expect(profileSelect.value).toBe('p1');

    const skillSelect = screen.getByLabelText('Kỹ năng chuyên môn') as HTMLSelectElement;
    expect(skillSelect.value).toBe('s1');

    const targetLangSelect = screen.getByLabelText('Ngôn ngữ dịch sang') as HTMLSelectElement;
    expect(targetLangSelect.value).toBe('vi');
  });

  it('calls onConfirm with modified configuration when clicking start', () => {
    const onConfirm = vi.fn();
    render(
      <RunConfigModal
        isOpen={true}
        onClose={vi.fn()}
        onConfirm={onConfirm}
        queuedJobs={mockQueuedJobs}
        general={mockGeneral}
        systemInfo={mockSystemInfo}
        profiles={mockProfiles}
        defaultProfileId="p1"
        presets={mockPresets}
        userSkills={[]}
        defaultSkillId={null}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );

    // Change target lang to 'en'
    const targetLangSelect = screen.getByLabelText('Ngôn ngữ dịch sang');
    fireEvent.change(targetLangSelect, { target: { value: 'en' } });

    // Change profile to 'p2'
    const profileSelect = screen.getByLabelText('AI Dịch thuật');
    fireEvent.change(profileSelect, { target: { value: 'p2' } });

    // Click confirm
    const startButton = screen.getByRole('button', { name: /Bắt đầu chạy/i });
    fireEvent.click(startButton);

    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onConfirm).toHaveBeenCalledWith({
      generalPatch: expect.objectContaining({
        target_lang: 'en',
        whisper_model: 'base',
      }),
      profileId: 'p2',
      skillId: null,
    });
  });

  it('calls onClose when clicking cancel or close button', () => {
    const onClose = vi.fn();
    render(
      <RunConfigModal
        isOpen={true}
        onClose={onClose}
        onConfirm={vi.fn()}
        queuedJobs={mockQueuedJobs}
        general={mockGeneral}
        systemInfo={mockSystemInfo}
        profiles={mockProfiles}
        defaultProfileId="p1"
        presets={mockPresets}
        userSkills={[]}
        defaultSkillId={null}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );

    const cancelBtn = screen.getByRole('button', { name: /Hủy bỏ/i });
    fireEvent.click(cancelBtn);
    expect(onClose).toHaveBeenCalledTimes(1);

    const closeBtn = screen.getByRole('button', { name: 'Đóng' });
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('shows download button when an un-downloaded whisper model is selected', () => {
    const onDownloadModel = vi.fn();
    render(
      <RunConfigModal
        isOpen={true}
        onClose={vi.fn()}
        onConfirm={vi.fn()}
        queuedJobs={mockQueuedJobs}
        general={mockGeneral}
        systemInfo={mockSystemInfo}
        profiles={mockProfiles}
        defaultProfileId="p1"
        presets={mockPresets}
        userSkills={[]}
        defaultSkillId={null}
        onDownloadModel={onDownloadModel}
        downloadingModel={null}
      />
    );

    const whisperSelect = screen.getByLabelText('Mô hình Whisper');
    fireEvent.change(whisperSelect, { target: { value: 'large-v3' } });

    const downloadBtn = screen.getByRole('button', { name: /Tải/i });
    expect(downloadBtn).toBeDefined();

    fireEvent.click(downloadBtn);
    expect(onDownloadModel).toHaveBeenCalledWith('large-v3');
  });
});
