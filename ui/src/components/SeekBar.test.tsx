import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { SeekBar } from './SeekBar';
import type { SubtitleSegment } from '../types';

const RECT = { left: 0, top: 0, width: 400, height: 10, right: 400, bottom: 10, x: 0, y: 0, toJSON: () => {} } as DOMRect;
const seg: SubtitleSegment = { id: 1, start_ms: 1000, end_ms: 3000, original_text: 'a', translated_text: 'b' };

afterEach(() => vi.restoreAllMocks());

describe('SeekBar', () => {
  it('click seeks to the right ms', () => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(RECT);
    const onSeek = vi.fn();
    render(<SeekBar durationMs={10000} segments={[]} currentTimeMs={0} onSeek={onSeek} />);
    fireEvent.click(screen.getByRole('slider'), { clientX: 200 });
    expect(onSeek).toHaveBeenCalledWith(5000);
    fireEvent.click(screen.getByRole('slider'), { clientX: 500 });
    expect(onSeek).toHaveBeenCalledWith(10000);
  });

  it('renders segment blocks at proportional positions and active highlight', () => {
    const { rerender } = render(<SeekBar durationMs={10000} segments={[seg]} currentTimeMs={0} onSeek={vi.fn()} />);
    const block = screen.getByTestId('seg-1');
    expect(block.style.left).toBe('10%');
    expect(block.style.width).toBe('20%');
    expect(block.className).toContain('slate-500');

    rerender(<SeekBar durationMs={10000} segments={[seg]} currentTimeMs={2000} onSeek={vi.fn()} />);
    expect(screen.getByTestId('seg-1').className).toContain('blue-400');
    expect(screen.getByTestId('playhead').style.left).toBe('20%');
  });

  it('ignores clicks when duration is 0', () => {
    const onSeek = vi.fn();
    render(<SeekBar durationMs={0} segments={[]} currentTimeMs={0} onSeek={onSeek} />);
    fireEvent.click(screen.getByRole('slider'), { clientX: 100 });
    expect(onSeek).not.toHaveBeenCalled();
  });
});
