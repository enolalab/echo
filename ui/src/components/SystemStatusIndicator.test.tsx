import React from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SystemStatusIndicator } from './SystemStatusIndicator';
import type { SystemInfo, ProfileView } from '../types';

describe('SystemStatusIndicator', () => {
  const baseSystemInfo: SystemInfo = {
    has_cuda: true,
    cuda_ready: true,
    cuda_error: null,
    ffmpeg_ready: true,
    ollama_running: true,
    ollama_models: ['qwen2.5:7b'],
    whisper_models: [
      { name: 'base', downloaded: true, size_mb: 142 },
      { name: 'small', downloaded: false, size_mb: 466 },
    ],
  };

  const baseProfiles: ProfileView[] = [
    {
      id: 'openai-1',
      name: 'OpenAI GPT-4o',
      kind: 'openai',
      base_url: 'https://api.openai.com/v1',
      model: 'gpt-4o',
      has_key: true,
    },
  ];

  it('renders "Sẵn sàng" when all dependencies and configs are ready', () => {
    render(
      <SystemStatusIndicator
        systemInfo={baseSystemInfo}
        whisperModel="base"
        profiles={baseProfiles}
        defaultProfileId="openai-1"
        preferGpu={true}
        onOpenSettings={vi.fn()}
      />
    );

    expect(screen.getByText('Sẵn sàng')).toBeTruthy();
  });

  it('shows "Chưa tải model" when selected Whisper model is not downloaded', () => {
    render(
      <SystemStatusIndicator
        systemInfo={baseSystemInfo}
        whisperModel="small"
        profiles={baseProfiles}
        defaultProfileId="openai-1"
        preferGpu={true}
        onOpenSettings={vi.fn()}
      />
    );

    expect(screen.getByText('Chưa tải model')).toBeTruthy();
  });

  it('shows "Cảnh báo GPU" when CUDA is selected but has driver error', () => {
    const brokenGpuInfo: SystemInfo = {
      ...baseSystemInfo,
      cuda_ready: false,
      cuda_error: 'NVIDIA-SMI failed',
    };

    render(
      <SystemStatusIndicator
        systemInfo={brokenGpuInfo}
        whisperModel="base"
        profiles={baseProfiles}
        defaultProfileId="openai-1"
        preferGpu={true}
        onOpenSettings={vi.fn()}
      />
    );

    expect(screen.getByText('Cảnh báo GPU')).toBeTruthy();
  });

  it('shows "Thiếu FFmpeg" when ffmpeg_ready is false', () => {
    const noFfmpegInfo: SystemInfo = {
      ...baseSystemInfo,
      ffmpeg_ready: false,
    };

    render(
      <SystemStatusIndicator
        systemInfo={noFfmpegInfo}
        whisperModel="base"
        profiles={baseProfiles}
        defaultProfileId="openai-1"
        preferGpu={true}
        onOpenSettings={vi.fn()}
      />
    );

    expect(screen.getByText('Thiếu FFmpeg')).toBeTruthy();
  });

  it('opens status checklist popover when clicked', () => {
    render(
      <SystemStatusIndicator
        systemInfo={baseSystemInfo}
        whisperModel="base"
        profiles={baseProfiles}
        defaultProfileId="openai-1"
        preferGpu={true}
        onOpenSettings={vi.fn()}
      />
    );

    const button = screen.getByTitle('Bấm để xem chi tiết trạng thái hệ thống');
    fireEvent.click(button);

    expect(screen.getByText('Trạng thái hệ thống')).toBeTruthy();
    expect(screen.getByText(/FFmpeg/i)).toBeTruthy();
    expect(screen.getByText(/Model Whisper: base/i)).toBeTruthy();
    expect(screen.getByText(/AI Dịch thuật/i)).toBeTruthy();
    expect(screen.getByText(/GPU CUDA/i)).toBeTruthy();
  });
});
