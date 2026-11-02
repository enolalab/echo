import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { YoutubeImportModal } from './YoutubeImportModal';
import * as ipc from '../lib/ipc';

vi.mock('../lib/ipc', () => ({
  inspectYoutubeUrl: vi.fn(),
  downloadYoutubeItems: vi.fn(),
  cancelYoutubeDownload: vi.fn(),
  listenYoutubeProgress: vi.fn(() => Promise.resolve(() => {})),
}));

describe('YoutubeImportModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders input field and inspect button when open', () => {
    render(<YoutubeImportModal isOpen={true} onClose={vi.fn()} onVideosDownloaded={vi.fn()} />);
    expect(screen.getByPlaceholderText(/dán link youtube/i)).toBeTruthy();
    expect(screen.getByRole('button', { name: /kiểm tra/i })).toBeTruthy();
  });

  it('inspects playlist and toggles selection', async () => {
    (ipc.inspectYoutubeUrl as any).mockResolvedValueOnce({
      is_playlist: true,
      playlist_id: 'PL1',
      title: 'Sample Course',
      items: [
        { id: 'v1', title: 'Lesson 1', duration_seconds: 120, webpage_url: 'url1' },
        { id: 'v2', title: 'Lesson 2', duration_seconds: 180, webpage_url: 'url2' },
      ],
    });

    render(<YoutubeImportModal isOpen={true} onClose={vi.fn()} onVideosDownloaded={vi.fn()} />);
    const input = screen.getByPlaceholderText(/dán link youtube/i);
    fireEvent.change(input, { target: { value: 'https://youtube.com/playlist?list=PL1' } });
    fireEvent.click(screen.getByRole('button', { name: /kiểm tra/i }));

    await waitFor(() => {
      expect(screen.getByText('Sample Course')).toBeTruthy();
      expect(screen.getByText('Lesson 1')).toBeTruthy();
      expect(screen.getByText('Lesson 2')).toBeTruthy();
    });

    // Check action button shows count
    expect(screen.getByText(/tải 2 video đã chọn/i)).toBeTruthy();
  });

  it('asks before re-downloading items already in the queue', async () => {
    (ipc.inspectYoutubeUrl as any).mockResolvedValueOnce({
      is_playlist: true,
      playlist_id: 'PL1',
      title: 'Sample Course',
      items: [
        { id: 'v1', title: 'Lesson 1', duration_seconds: 120, webpage_url: 'url1' },
        { id: 'v2', title: 'Lesson 2', duration_seconds: 180, webpage_url: 'url2' },
      ],
    });
    (ipc.downloadYoutubeItems as any).mockResolvedValueOnce(undefined);

    render(
      <YoutubeImportModal
        isOpen={true}
        onClose={vi.fn()}
        onVideosDownloaded={vi.fn()}
        existingIds={new Set(['v1'])}
      />
    );
    fireEvent.change(screen.getByPlaceholderText(/dán link youtube/i), {
      target: { value: 'https://youtube.com/playlist?list=PL1' },
    });
    fireEvent.click(screen.getByRole('button', { name: /kiểm tra/i }));

    await waitFor(() => expect(screen.getByText('Sample Course')).toBeTruthy());
    // dupe badge visible on the queued item
    expect(screen.getByText(/đã có trong hàng đợi/i)).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: /tải 2 video đã chọn/i }));
    expect(await screen.findByText(/bỏ qua chúng và chỉ tải 1 video còn lại/i)).toBeTruthy();
    expect(ipc.downloadYoutubeItems).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: /bỏ qua — tải 1 video/i }));
    await waitFor(() => expect(ipc.downloadYoutubeItems).toHaveBeenCalledTimes(1));
    const arg = (ipc.downloadYoutubeItems as any).mock.calls[0][0];
    expect(arg.items.map((i: any) => i.id)).toEqual(['v2']);
  });

  it('"vẫn tải tất cả" downloads every selected item including dupes', async () => {
    (ipc.inspectYoutubeUrl as any).mockResolvedValueOnce({
      is_playlist: true,
      playlist_id: 'PL1',
      title: 'Sample Course',
      items: [
        { id: 'v1', title: 'Lesson 1', duration_seconds: 120, webpage_url: 'url1' },
        { id: 'v2', title: 'Lesson 2', duration_seconds: 180, webpage_url: 'url2' },
      ],
    });
    (ipc.downloadYoutubeItems as any).mockResolvedValueOnce(undefined);

    render(
      <YoutubeImportModal
        isOpen={true}
        onClose={vi.fn()}
        onVideosDownloaded={vi.fn()}
        existingIds={new Set(['v1'])}
      />
    );
    fireEvent.change(screen.getByPlaceholderText(/dán link youtube/i), {
      target: { value: 'https://youtube.com/playlist?list=PL1' },
    });
    fireEvent.click(screen.getByRole('button', { name: /kiểm tra/i }));
    await waitFor(() => expect(screen.getByText('Sample Course')).toBeTruthy());

    fireEvent.click(screen.getByRole('button', { name: /tải 2 video đã chọn/i }));
    fireEvent.click(await screen.findByRole('button', { name: /vẫn tải tất cả/i }));
    await waitFor(() => expect(ipc.downloadYoutubeItems).toHaveBeenCalledTimes(1));
    const arg = (ipc.downloadYoutubeItems as any).mock.calls[0][0];
    expect(arg.items.map((i: any) => i.id)).toEqual(['v1', 'v2']);
  });
});
