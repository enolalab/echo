import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { ExportModal } from './ExportModal';

const segments = [
  {
    id: 1,
    start_ms: 1000,
    end_ms: 3000,
    original_text: 'Hello',
    translated_text: 'Xin chào',
  },
];

const baseProps = {
  isOpen: true,
  onClose: vi.fn(),
  videoPath: '/path/video.mp4',
  defaultOutputPath: '/path/video_subtitled.mp4',
  segments,
  initialPreset: 'youtube_yellow' as const,
  initialBilingual: true,
};

describe('ExportModal', () => {
  it('enqueues a background export request and closes on start', () => {
    const onStartExport = vi.fn();
    const onClose = vi.fn();

    render(
      <ExportModal {...baseProps} onClose={onClose} onStartExport={onStartExport} />
    );

    fireEvent.click(screen.getByRole('button', { name: /Bắt đầu xử lý/i }));

    expect(onStartExport).toHaveBeenCalledTimes(1);
    const requests = onStartExport.mock.calls[0][0];
    expect(requests).toHaveLength(1);
    expect(requests[0]).toEqual(
      expect.objectContaining({
        video_path: '/path/video.mp4',
        output_path: '/path/video_subtitled.mp4',
        mode: 'hardsub',
        bilingual: true,
        style_preset: 'youtube_yellow',
      })
    );
    expect(requests[0].job_id).toMatch(/^export-/);
    // Modal hands off and closes — export runs as a background job
    expect(onClose).toHaveBeenCalled();
  });

  it('builds one request per item in batch mode', () => {
    const onStartExport = vi.fn();
    const batchItems = [
      { videoPath: '/path/b.mp4', segments },
      { videoPath: '/path/c.mp4', segments },
    ];

    render(
      <ExportModal
        {...baseProps}
        batchItems={batchItems}
        onStartExport={onStartExport}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /Tất cả 3 video/i }));
    fireEvent.click(screen.getByRole('button', { name: /Bắt đầu xử lý \(3 video\)/i }));

    const requests = onStartExport.mock.calls[0][0];
    expect(requests).toHaveLength(3);
    expect(requests.map((r: { video_path: string }) => r.video_path)).toEqual([
      '/path/video.mp4',
      '/path/b.mp4',
      '/path/c.mp4',
    ]);
    // Batch outputs derive their own file names in the chosen directory
    expect(requests[1].output_path).toBe('/path/b_subtitled.mp4');
  });
});
