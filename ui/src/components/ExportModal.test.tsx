import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { ExportModal } from './ExportModal';

describe('ExportModal', () => {
  it('triggers export callback with chosen mode and style preset', () => {
    const onExport = vi.fn().mockResolvedValue('Thành công');
    const segments = [
      {
        id: 1,
        start_ms: 1000,
        end_ms: 3000,
        original_text: 'Hello',
        translated_text: 'Xin chào',
      },
    ];

    render(
      <ExportModal
        isOpen={true}
        onClose={vi.fn()}
        videoPath="/path/video.mp4"
        defaultOutputPath="/path/video_subtitled.mp4"
        segments={segments}
        initialPreset="youtube_yellow"
        initialBilingual={true}
        onExport={onExport}
        onOpenFolder={vi.fn()}
      />
    );

    const renderBtn = screen.getByRole('button', { name: /Render & Xuất Ngay/i });
    fireEvent.click(renderBtn);

    expect(onExport).toHaveBeenCalledWith(
      expect.objectContaining({
        video_path: '/path/video.mp4',
        mode: 'hardsub',
        bilingual: true,
        style_preset: 'youtube_yellow',
      })
    );
  });
});
