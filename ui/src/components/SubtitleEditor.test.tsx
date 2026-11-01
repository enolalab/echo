import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { SubtitleEditor } from './SubtitleEditor';

describe('SubtitleEditor', () => {
  it('clicking a row calls onSeek with segment start_ms', () => {
    const onSeek = vi.fn();
    const segments = [
      {
        id: 1,
        start_ms: 1500,
        end_ms: 3500,
        original_text: 'Hello world',
        translated_text: 'Xin chào thế giới',
      },
    ];

    render(
      <SubtitleEditor
        segments={segments}
        currentTimeMs={0}
        onSeek={onSeek}
        onUpdateSegment={vi.fn()}
        onDeleteSegment={vi.fn()}
        onSplitSegment={vi.fn()}
        onMergeSegment={vi.fn()}
        onAddSegment={vi.fn()}
        onBatchReplace={vi.fn()}
      />
    );

    const row = screen.getByText('#1');
    fireEvent.click(row);
    expect(onSeek).toHaveBeenCalledWith(1500);
  });

  it('filters rows based on search input', () => {
    const segments = [
      { id: 1, start_ms: 1000, end_ms: 2000, original_text: 'First line', translated_text: 'Dòng một' },
      { id: 2, start_ms: 2500, end_ms: 4000, original_text: 'Second sentence', translated_text: 'Dòng hai' },
    ];

    render(
      <SubtitleEditor
        segments={segments}
        currentTimeMs={0}
        onSeek={vi.fn()}
        onUpdateSegment={vi.fn()}
        onDeleteSegment={vi.fn()}
        onSplitSegment={vi.fn()}
        onMergeSegment={vi.fn()}
        onAddSegment={vi.fn()}
        onBatchReplace={vi.fn()}
      />
    );

    expect(screen.getByText('#1')).toBeDefined();
    expect(screen.getByText('#2')).toBeDefined();

    const searchInput = screen.getByPlaceholderText('Tìm kiếm phụ đề...');
    fireEvent.change(searchInput, { target: { value: 'First' } });

    expect(screen.getByText('#1')).toBeDefined();
    expect(screen.queryByText('#2')).toBeNull();
  });
});
