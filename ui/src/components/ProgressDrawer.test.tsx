import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ProgressDrawer } from './ProgressDrawer';

describe('ProgressDrawer', () => {
  it('renders progress bar with correct width style and percentage badge', () => {
    render(
      <ProgressDrawer
        currentStage={{ stage: 'translate', message: 'Đang dịch...' }}
        progress={{
          stage: 'translate',
          message: 'Đang dịch 18/45 đoạn...',
          percentage: 42.5,
          current_step: 18,
          total_steps: 45,
          elapsed_secs: 28,
        }}
        logs={[]}
        isRunning={true}
        onOpenExport={vi.fn()}
        canExport={false}
      />
    );

    const progressLine = screen.getByTestId('progress-line');
    expect(progressLine.style.width).toBe('42.5%');

    const badge = screen.getByTestId('progress-percentage-badge');
    expect(badge.textContent).toBe('43%');

    const timer = screen.getByTestId('progress-timer');
    expect(timer.textContent).toContain('00:28');

    expect(screen.getByText('Đang dịch 18/45 đoạn...')).toBeDefined();
  });

  it('renders ready state with 100% progress line when complete', () => {
    render(
      <ProgressDrawer
        currentStage={{ stage: 'ready', message: 'Hoàn tất!' }}
        progress={{
          stage: 'ready',
          message: 'Hoàn tất! Đã tạo 45 đoạn phụ đề.',
          percentage: 100,
          current_step: 45,
          total_steps: 45,
          elapsed_secs: 65,
        }}
        logs={['Log 1', 'Log 2']}
        isRunning={false}
        onOpenExport={vi.fn()}
        canExport={true}
      />
    );

    const progressLine = screen.getByTestId('progress-line');
    expect(progressLine.style.width).toBe('100%');
    expect(progressLine.className).toContain('bg-emerald-500');
    expect(screen.getByText('Hoàn tất! Đã tạo 45 đoạn phụ đề.')).toBeDefined();
  });
});
