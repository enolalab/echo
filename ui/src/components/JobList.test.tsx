import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { JobList } from './JobList';
import type { BatchJob } from '../types';

const baseJob: BatchJob = {
  id: 'job-1',
  videoPath: '/videos/movie.mp4',
  status: 'queued',
  progress: null,
  segments: [],
  outputVideoPath: null,
  error: null,
};

const handlers = {
  onSelect: vi.fn(),
  onRemove: vi.fn(),
  onCancelJob: vi.fn(),
  onOpenFolder: vi.fn(),
};

describe('JobList', () => {
  it('shows empty state when no jobs', () => {
    render(<JobList jobs={[]} selectedJobId={null} {...handlers} />);
    expect(screen.getByText(/Chưa có file nào/i)).toBeDefined();
  });

  it('renders multiple jobs with statuses and output path', () => {
    const jobs: BatchJob[] = [
      baseJob,
      {
        ...baseJob,
        id: 'job-2',
        videoPath: '/videos/clip.mkv',
        status: 'running',
        progress: {
          job_id: 'job-2',
          stage: 'stt',
          message: 'Đang nhận dạng...',
          percentage: 42,
          elapsed_secs: 5,
        },
      },
      {
        ...baseJob,
        id: 'job-3',
        videoPath: '/videos/done.mp4',
        status: 'done',
        outputVideoPath: '/videos/done_subtitled.mp4',
      },
    ];
    render(<JobList jobs={jobs} selectedJobId="job-2" {...handlers} />);

    expect(screen.getByText('movie.mp4')).toBeDefined();
    expect(screen.getByText('clip.mkv')).toBeDefined();
    expect(screen.getByText('done.mp4')).toBeDefined();
    expect(screen.getByText('42%')).toBeDefined();
    expect(screen.getByText('→ done_subtitled.mp4')).toBeDefined();
  });

  it('selects a job on click and cancels running job', () => {
    const jobs: BatchJob[] = [
      { ...baseJob, status: 'running', progress: null },
      { ...baseJob, id: 'job-2', videoPath: '/v/b.mp4' },
    ];
    render(<JobList jobs={jobs} selectedJobId={null} {...handlers} />);

    fireEvent.click(screen.getByText('b.mp4'));
    expect(handlers.onSelect).toHaveBeenCalledWith('job-2');

    fireEvent.click(screen.getByTitle('Hủy file này'));
    expect(handlers.onCancelJob).toHaveBeenCalledWith('job-1');
  });
});
