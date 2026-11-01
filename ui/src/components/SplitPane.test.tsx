import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { SplitPane } from './SplitPane';

const RECT = { left: 0, top: 0, width: 1000, height: 1000, right: 1000, bottom: 1000, x: 0, y: 0, toJSON: () => {} } as DOMRect;

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(RECT);
  localStorage.clear();
});
afterEach(() => vi.restoreAllMocks());

const renderPane = (props: Partial<React.ComponentProps<typeof SplitPane>> = {}) =>
  render(
    <SplitPane direction="horizontal" initialRatio={0.5} minA={100} minB={100} {...props}>
      <div>A</div>
      <div>B</div>
    </SplitPane>
  );

describe('SplitPane', () => {
  it('drags the divider and updates ratio with min clamp', () => {
    renderPane();
    const sep = screen.getByRole('separator');
    const panelA = screen.getByTestId('split-a');
    expect(panelA.style.flexBasis).toBe('50%');

    fireEvent.mouseDown(sep);
    fireEvent.mouseMove(document, { clientX: 700 });
    expect(panelA.style.flexBasis).toBe('70%');

    // drag beyond minA (100px / 1000px = 10%) → clamps to 10%
    fireEvent.mouseMove(document, { clientX: 50 });
    expect(panelA.style.flexBasis).toBe('10%');
    fireEvent.mouseUp(document);
  });

  it('persists ratio to localStorage on mouseup when storageKey set', () => {
    renderPane({ storageKey: 'main' });
    fireEvent.mouseDown(screen.getByRole('separator'));
    fireEvent.mouseMove(document, { clientX: 700 });
    fireEvent.mouseUp(document);
    expect(localStorage.getItem('echo.layout.main')).toBe('0.7');
  });

  it('loads persisted ratio on mount', () => {
    localStorage.setItem('echo.layout.main', '0.3');
    renderPane({ storageKey: 'main' });
    expect(screen.getByTestId('split-a').style.flexBasis).toBe('30%');
  });

  it('falls back to initialRatio on corrupted storage', () => {
    localStorage.setItem('echo.layout.main', 'garbage');
    renderPane({ storageKey: 'main' });
    expect(screen.getByTestId('split-a').style.flexBasis).toBe('50%');
  });

  it('vertical direction uses clientY', () => {
    renderPane({ direction: 'vertical', initialRatio: 0.5 });
    fireEvent.mouseDown(screen.getByRole('separator'));
    fireEvent.mouseMove(document, { clientY: 800 });
    expect(screen.getByTestId('split-a').style.flexBasis).toBe('80%');
    fireEvent.mouseUp(document);
  });
});
