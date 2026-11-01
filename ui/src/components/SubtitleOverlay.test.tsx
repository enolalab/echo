import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { SubtitleOverlay } from './SubtitleOverlay';

describe('SubtitleOverlay', () => {
  it('displays active subtitle text at matching timestamp', () => {
    const segments = [
      {
        id: 1,
        start_ms: 1000,
        end_ms: 4000,
        original_text: 'Hello world',
        translated_text: 'Xin chào thế giới',
      },
    ];

    render(
      <SubtitleOverlay
        currentTimeMs={2500}
        segments={segments}
        preset="youtube_yellow"
        isBilingual={true}
      />
    );

    expect(screen.getByText('Xin chào thế giới')).toBeDefined();
    expect(screen.getByText('Hello world')).toBeDefined();
  });

  it('renders nothing when currentTimeMs is outside segments', () => {
    const segments = [
      {
        id: 1,
        start_ms: 1000,
        end_ms: 4000,
        original_text: 'Hello world',
        translated_text: 'Xin chào thế giới',
      },
    ];

    const { container } = render(
      <SubtitleOverlay
        currentTimeMs={5000}
        segments={segments}
        preset="youtube_yellow"
        isBilingual={true}
      />
    );

    expect(container.firstChild).toBeNull();
  });
});
