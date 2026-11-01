import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { HeaderBar } from './HeaderBar';

describe('HeaderBar', () => {
  it('renders Start button disabled when no video is selected', () => {
    render(
      <HeaderBar
        videoPath=""
        onSelectVideo={vi.fn()}
        whisperModel="base"
        onSelectWhisperModel={vi.fn()}
        ollamaModel="qwen2.5:7b"
        onSelectOllamaModel={vi.fn()}
        targetLang="vi"
        onSelectTargetLang={vi.fn()}
        bilingual={true}
        onToggleBilingual={vi.fn()}
        stylePreset="youtube_yellow"
        onSelectPreset={vi.fn()}
        isRunning={false}
        onStart={vi.fn()}
        onCancel={vi.fn()}
        systemInfo={null}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );
    const startBtn = screen.getByRole('button', { name: /Bắt Đầu/i }) as HTMLButtonElement;
    expect(startBtn.disabled).toBe(true);
  });

  it('renders Start button enabled when video is selected', () => {
    render(
      <HeaderBar
        videoPath="/path/to/movie.mp4"
        onSelectVideo={vi.fn()}
        whisperModel="base"
        onSelectWhisperModel={vi.fn()}
        ollamaModel="qwen2.5:7b"
        onSelectOllamaModel={vi.fn()}
        targetLang="vi"
        onSelectTargetLang={vi.fn()}
        bilingual={true}
        onToggleBilingual={vi.fn()}
        stylePreset="youtube_yellow"
        onSelectPreset={vi.fn()}
        isRunning={false}
        onStart={vi.fn()}
        onCancel={vi.fn()}
        systemInfo={null}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );
    const startBtn = screen.getByRole('button', { name: /Bắt Đầu/i }) as HTMLButtonElement;
    expect(startBtn.disabled).toBe(false);
    expect(screen.getByText('movie.mp4')).toBeDefined();
  });
});
