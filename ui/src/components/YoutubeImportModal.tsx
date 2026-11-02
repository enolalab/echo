import React, { useState, useEffect, useRef } from 'react';
import {
  X,
  Youtube,
  Loader2,
  ClipboardPaste,
  AlertCircle,
  CheckCircle2,
  Download,
  Film,
} from 'lucide-react';
import type {
  YoutubeInspectResult,
  YoutubeProgressPayload,
  YoutubeVideoItem,
} from '../types';
import * as ipc from '../lib/ipc';

interface YoutubeImportModalProps {
  isOpen: boolean;
  onClose: () => void;
  onVideosDownloaded: (savedPaths: string[]) => void;
  defaultDownloadDir?: string;
}

type Stage = 'input' | 'select' | 'downloading';

function formatDuration(seconds?: number | null): string {
  if (!seconds || seconds <= 0) return '--:--';
  const s = Math.floor(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) return `${h}:${m.toString().padStart(2, '0')}:${sec.toString().padStart(2, '0')}`;
  return `${m}:${sec.toString().padStart(2, '0')}`;
}

function friendlyInspectError(raw: string): string {
  const lower = raw.toLowerCase();
  if (lower.includes('private') || lower.includes('not found') || lower.includes('404')) {
    return 'Video không tồn tại hoặc đã bị đặt ở chế độ riêng tư.';
  }
  if (lower.includes('age') || lower.includes('sign in')) {
    return 'Video yêu cầu xác minh độ tuổi hoặc đăng nhập.';
  }
  if (lower.includes('yt-dlp')) return raw;
  return `Không kiểm tra được link: ${raw}`;
}

export const YoutubeImportModal: React.FC<YoutubeImportModalProps> = ({
  isOpen,
  onClose,
  onVideosDownloaded,
  defaultDownloadDir,
}) => {
  const [stage, setStage] = useState<Stage>('input');
  const [url, setUrl] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const [ytDlpReady, setYtDlpReady] = useState<boolean | null>(null);
  const [result, setResult] = useState<YoutubeInspectResult | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [progressMap, setProgressMap] = useState<Record<string, YoutubeProgressPayload>>({});
  const [downloadItems, setDownloadItems] = useState<YoutubeVideoItem[]>([]);
  const [downloadFinished, setDownloadFinished] = useState(false);
  const wasOpen = useRef(false);

  // Reset transient state each time the modal is reopened
  useEffect(() => {
    if (isOpen && !wasOpen.current) {
      setStage('input');
      setError(null);
      setProgressMap({});
      setDownloadFinished(false);
    }
    wasOpen.current = isOpen;
  }, [isOpen]);

  // Check yt-dlp availability once when opened; auto-install if missing
  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    const probe = async () => {
      try {
        const status = await ipc.checkYtDlpStatus?.();
        if (cancelled) return;
        if (status?.installed) {
          setYtDlpReady(true);
        } else {
          setYtDlpReady(false);
          const ensured = await ipc.ensureYtDlp?.();
          if (!cancelled) setYtDlpReady(ensured?.installed ?? false);
        }
      } catch {
        if (!cancelled) setYtDlpReady(false);
      }
    };
    probe();
    return () => {
      cancelled = true;
    };
  }, [isOpen]);

  // Subscribe to download progress for the whole component lifetime so that
  // background downloads (modal closed) still queue files into the JobList.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    ipc.listenYoutubeProgress?.((payload) => {
      setProgressMap((prev) => ({ ...prev, [payload.video_id]: payload }));
      if (payload.stage === 'completed' && payload.saved_path) {
        onVideosDownloaded([payload.saved_path]);
      }
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      if (unlisten) unlisten();
    };
  }, [onVideosDownloaded]);

  const handlePaste = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) setUrl(text.trim());
    } catch {
      // Clipboard unavailable — user can still paste manually
    }
  };

  const handleInspect = async (inspectUrl?: string) => {
    const target = (inspectUrl ?? url).trim();
    if (!target || inspecting) return;
    setInspecting(true);
    setError(null);
    try {
      const res = await ipc.inspectYoutubeUrl(target);
      setResult(res);
      setSelected(new Set(res.items.map((i) => i.id)));
      setStage('select');
    } catch (err) {
      setError(friendlyInspectError(String(err)));
    } finally {
      setInspecting(false);
    }
  };

  const videoIdInUrl = (() => {
    const m = url.match(/[?&]v=([a-zA-Z0-9_-]{11})/);
    return m?.[1] ?? null;
  })();
  const hasBothVideoAndPlaylist = Boolean(videoIdInUrl && /[?&]list=/.test(url));

  const toggleItem = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleDownload = async () => {
    if (!result) return;
    const items = result.items.filter((i) => selected.has(i.id));
    if (items.length === 0) return;
    setDownloadItems(items);
    setProgressMap({});
    setDownloadFinished(false);
    setStage('downloading');
    try {
      await ipc.downloadYoutubeItems({
        items,
        output_dir: defaultDownloadDir ?? null,
      });
    } catch (err) {
      setError(String(err));
    } finally {
      setDownloadFinished(true);
    }
  };

  const handleCancelDownload = () => {
    ipc.cancelYoutubeDownload().catch(() => {});
  };

  if (!isOpen) return null;

  const completedCount = downloadItems.filter(
    (i) => progressMap[i.id]?.stage === 'completed'
  ).length;
  const selectedCount = selected.size;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="w-full max-w-2xl max-h-[85vh] flex flex-col bg-slate-900 border border-slate-700 rounded-xl shadow-2xl overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-slate-800 shrink-0">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-red-600/15 border border-red-500/40 flex items-center justify-center">
              <Youtube className="w-4 h-4 text-red-400" />
            </div>
            <div>
              <div className="text-sm font-semibold text-slate-100">Nhập từ YouTube</div>
              <div className="text-[11px] text-slate-500">
                Tải video hoặc playlist về máy và đưa vào hàng đợi
              </div>
            </div>
          </div>
          <button
            onClick={onClose}
            aria-label="Đóng"
            className="w-8 h-8 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* yt-dlp status banner */}
        {ytDlpReady === false && (
          <div className="px-4 py-2 bg-amber-950/40 border-b border-amber-800/40 text-[11px] text-amber-300 flex items-center gap-2 shrink-0">
            <Loader2 className="w-3.5 h-3.5 animate-spin shrink-0" />
            Đang tải yt-dlp lần đầu, vui lòng chờ...
          </div>
        )}

        {/* Body */}
        <div className="flex-1 min-h-0 overflow-y-auto p-4">
          {stage === 'input' && (
            <div className="flex flex-col gap-3">
              <div className="flex gap-2">
                <div className="relative flex-1">
                  <input
                    type="text"
                    value={url}
                    onChange={(e) => setUrl(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && handleInspect()}
                    placeholder="Dán link YouTube (video hoặc playlist)..."
                    className="w-full h-10 pl-3 pr-10 rounded-lg bg-slate-950 border border-slate-700 text-sm text-slate-100 placeholder:text-slate-600 focus:outline-none focus:border-red-500 focus:ring-1 focus:ring-red-500/40"
                  />
                  <button
                    onClick={handlePaste}
                    title="Dán từ clipboard"
                    aria-label="Dán từ clipboard"
                    className="absolute right-1.5 top-1/2 -translate-y-1/2 w-7 h-7 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
                  >
                    <ClipboardPaste className="w-4 h-4" />
                  </button>
                </div>
                <button
                  onClick={() => handleInspect()}
                  disabled={!url.trim() || inspecting}
                  className="h-10 px-4 flex items-center gap-2 bg-red-600 hover:bg-red-500 disabled:bg-slate-800 disabled:text-slate-500 text-white text-sm font-semibold rounded-lg transition-colors shrink-0"
                >
                  {inspecting ? <Loader2 className="w-4 h-4 animate-spin" /> : null}
                  Kiểm tra
                </button>
              </div>

              {error && (
                <div className="flex items-start gap-2 px-3 py-2.5 rounded-lg bg-rose-950/50 border border-rose-800/50 text-rose-300 text-xs">
                  <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
                  <span>{error}</span>
                </div>
              )}

              <div className="text-[11px] text-slate-500 leading-relaxed">
                Hỗ trợ link video (`watch?v=...`, `youtu.be/...`, `shorts/...`) và playlist
                (`playlist?list=...`). Video sẽ được tải về dạng MP4 rồi tự động thêm vào hàng đợi
                tạo phụ đề.
              </div>
            </div>
          )}

          {stage === 'select' && result && (
            <div className="flex flex-col gap-3">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="text-sm font-semibold text-slate-100 truncate">{result.title}</div>
                  <div className="text-[11px] text-slate-500">
                    {result.is_playlist
                      ? `Playlist · Đã chọn ${selectedCount}/${result.items.length} video`
                      : 'Video đơn'}
                  </div>
                </div>
                <button
                  onClick={() => setStage('input')}
                  className="text-[11px] text-slate-400 hover:text-slate-200 shrink-0"
                >
                  ← Đổi link
                </button>
              </div>

              {hasBothVideoAndPlaylist && result.is_playlist && videoIdInUrl && (
                <div className="flex items-center justify-between gap-2 px-3 py-2 rounded-lg bg-blue-950/40 border border-blue-800/40 text-[11px] text-blue-300">
                  <span>Link này chứa cả video lẫn playlist — đang hiển thị playlist.</span>
                  <button
                    onClick={() =>
                      handleInspect(`https://www.youtube.com/watch?v=${videoIdInUrl}`)
                    }
                    className="font-semibold hover:text-blue-100 shrink-0"
                  >
                    Chỉ tải video hiện tại
                  </button>
                </div>
              )}

              {result.is_playlist && (
                <div className="flex items-center gap-2 text-[11px]">
                  <button
                    onClick={() => setSelected(new Set(result.items.map((i) => i.id)))}
                    className="px-2.5 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 transition-colors"
                  >
                    Chọn tất cả
                  </button>
                  <button
                    onClick={() => setSelected(new Set())}
                    className="px-2.5 py-1 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 transition-colors"
                  >
                    Bỏ chọn tất cả
                  </button>
                </div>
              )}

              <div className="flex flex-col gap-1.5 max-h-[42vh] overflow-y-auto pr-1">
                {result.items.map((item, idx) => (
                  <label
                    key={item.id}
                    className={`flex items-center gap-3 px-2.5 py-2 rounded-lg border cursor-pointer transition-colors ${
                      selected.has(item.id)
                        ? 'bg-slate-800/70 border-slate-600'
                        : 'bg-slate-900/40 border-slate-800 hover:border-slate-700'
                    }`}
                  >
                    {result.is_playlist && (
                      <input
                        type="checkbox"
                        checked={selected.has(item.id)}
                        onChange={() => toggleItem(item.id)}
                        className="w-4 h-4 accent-red-500 shrink-0"
                      />
                    )}
                    <span className="w-6 text-[11px] text-slate-500 text-right shrink-0">
                      {idx + 1}
                    </span>
                    {item.thumbnail_url ? (
                      <img
                        src={item.thumbnail_url}
                        alt=""
                        className="w-20 aspect-video object-cover rounded shrink-0 bg-slate-800"
                      />
                    ) : (
                      <div className="w-20 aspect-video rounded shrink-0 bg-slate-800 flex items-center justify-center">
                        <Film className="w-4 h-4 text-slate-600" />
                      </div>
                    )}
                    <div className="min-w-0 flex-1">
                      <div className="text-xs font-medium text-slate-200 line-clamp-2">
                        {item.title}
                      </div>
                      <div className="text-[10px] text-slate-500 truncate">
                        {item.channel_name ? `${item.channel_name} · ` : ''}
                        {formatDuration(item.duration_seconds)}
                      </div>
                    </div>
                  </label>
                ))}
              </div>
            </div>
          )}

          {stage === 'downloading' && (
            <div className="flex flex-col gap-3">
              <div className="flex items-center justify-between text-xs">
                <span className="text-slate-300 font-medium">
                  {downloadFinished
                    ? `Hoàn tất ${completedCount}/${downloadItems.length} video`
                    : `Đang tải ${completedCount}/${downloadItems.length} video hoàn tất`}
                </span>
                {!downloadFinished && (
                  <span className="text-slate-500">
                    {progressMap[downloadItems.find((i) => progressMap[i.id]?.stage === 'downloading')?.id ?? '']?.speed ?? ''}
                  </span>
                )}
              </div>

              <div className="h-1.5 rounded-full bg-slate-800 overflow-hidden">
                <div
                  className="h-full bg-red-500 transition-all duration-300"
                  style={{
                    width: `${downloadItems.length ? (completedCount / downloadItems.length) * 100 : 0}%`,
                  }}
                />
              </div>

              <div className="flex flex-col gap-1.5 max-h-[40vh] overflow-y-auto pr-1">
                {downloadItems.map((item) => {
                  const p = progressMap[item.id];
                  const status = p?.stage;
                  return (
                    <div
                      key={item.id}
                      className="px-2.5 py-2 rounded-lg bg-slate-900/60 border border-slate-800"
                    >
                      <div className="flex items-center gap-2">
                        {status === 'completed' ? (
                          <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
                        ) : status === 'error' ? (
                          <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
                        ) : status === 'downloading' ? (
                          <Loader2 className="w-4 h-4 text-red-400 animate-spin shrink-0" />
                        ) : (
                          <Download className="w-4 h-4 text-slate-500 shrink-0" />
                        )}
                        <div className="min-w-0 flex-1 text-xs text-slate-300 truncate">
                          {item.title}
                        </div>
                        <div className="text-[10px] text-slate-500 shrink-0 tabular-nums">
                          {status === 'downloading'
                            ? `${Math.floor(p?.percentage ?? 0)}%${p?.eta ? ` · ${p.eta}` : ''}`
                            : status === 'completed'
                              ? 'Xong'
                              : status === 'error'
                                ? 'Lỗi'
                                : 'Chờ'}
                        </div>
                      </div>
                      {status === 'downloading' && (
                        <div className="mt-1.5 h-1 rounded-full bg-slate-800 overflow-hidden">
                          <div
                            className="h-full bg-red-500 transition-all duration-200"
                            style={{ width: `${p?.percentage ?? 0}%` }}
                          />
                        </div>
                      )}
                      {status === 'error' && p?.error && (
                        <div className="mt-1 text-[10px] text-rose-400 truncate">{p.error}</div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-slate-800 shrink-0">
          {stage === 'input' && (
            <button
              onClick={onClose}
              className="h-9 px-4 text-sm text-slate-400 hover:text-white rounded-lg transition-colors"
            >
              Đóng
            </button>
          )}

          {stage === 'select' && (
            <button
              onClick={handleDownload}
              disabled={selectedCount === 0}
              className="h-9 px-4 flex items-center gap-2 bg-red-600 hover:bg-red-500 disabled:bg-slate-800 disabled:text-slate-500 text-white text-sm font-semibold rounded-lg transition-colors"
            >
              <Download className="w-4 h-4" />
              {result && !result.is_playlist ? 'Tải video' : `Tải ${selectedCount} video đã chọn`}
            </button>
          )}

          {stage === 'downloading' && !downloadFinished && (
            <>
              <button
                onClick={handleCancelDownload}
                className="h-9 px-4 text-sm bg-rose-600/20 hover:bg-rose-600/30 border border-rose-700/50 text-rose-300 rounded-lg transition-colors"
              >
                Hủy tải
              </button>
              <button
                onClick={onClose}
                className="h-9 px-4 text-sm bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg transition-colors"
              >
                Chạy ngầm
              </button>
            </>
          )}

          {stage === 'downloading' && downloadFinished && (
            <button
              onClick={onClose}
              className="h-9 px-4 text-sm bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg transition-colors"
            >
              Đóng
            </button>
          )}
        </div>
      </div>
    </div>
  );
};
