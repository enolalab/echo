import React, { useState, useEffect, useMemo } from 'react';
import {
  X,
  Film,
  Layers,
  FileCode,
  Folder,
  Mic,
  Play,
} from 'lucide-react';
import type {
  ExportMode,
  StylePreset,
  ExportRequest,
  SubtitleSegment,
  DubOptions,
  RenderSpeed,
} from '../types';
import { pickSaveFile } from '../lib/ipc';

// Unique job ids across separate "Bắt đầu" clicks within the same ms
let reqCounter = 0;

interface ExportModalProps {
  isOpen: boolean;
  onClose: () => void;
  videoPath: string;
  defaultOutputPath: string;
  segments: SubtitleSegment[];
  initialPreset: StylePreset;
  initialBilingual: boolean;
  initialRenderSpeed?: RenderSpeed;
  initialPadSubs?: boolean;
  dubDefaults?: Omit<DubOptions, 'enabled'>;
  /** Other finished jobs eligible for batch export (deduped vs current video). */
  batchItems?: { videoPath: string; segments: SubtitleSegment[] }[];
  /** Queue the built export requests — they run as background jobs. */
  onStartExport: (requests: ExportRequest[]) => void;
}

const EXPORT_MODES: {
  id: ExportMode;
  title: string;
  desc: string;
  badge?: string;
  icon: typeof Film;
  color: string;
}[] = [
  {
    id: 'hardsub',
    title: 'Hardsub',
    desc: 'Gắn cứng phụ đề trực tiếp vào video. Tương thích mọi thiết bị và nền tảng.',
    badge: 'Khuyên dùng',
    icon: Film,
    color: 'text-amber-400',
  },
  {
    id: 'softsub',
    title: 'Softsub',
    desc: 'Nhúng track phụ đề vào container MP4/MKV. Bật/tắt được và render tức thì.',
    badge: 'Siêu nhanh',
    icon: Layers,
    color: 'text-indigo-400',
  },
  {
    id: 'subs_only',
    title: 'Subs-only',
    desc: 'Chỉ xuất riêng các file phụ đề độc lập (.srt và .ass) mà không can thiệp video.',
    icon: FileCode,
    color: 'text-cyan-400',
  },
];

export const ExportModal: React.FC<ExportModalProps> = ({
  isOpen,
  onClose,
  videoPath,
  defaultOutputPath,
  segments,
  initialPreset,
  initialBilingual,
  initialRenderSpeed,
  initialPadSubs,
  dubDefaults,
  batchItems,
  onStartExport,
}) => {
  const [mode, setMode] = useState<ExportMode>('hardsub');
  const [outputPath, setOutputPath] = useState(defaultOutputPath);
  const [stylePreset, setStylePreset] = useState<StylePreset>(initialPreset);
  const [bilingual, setBilingual] = useState(initialBilingual);
  const [renderSpeed, setRenderSpeed] = useState<RenderSpeed>(initialRenderSpeed ?? 'balanced');
  const [padSubs, setPadSubs] = useState(!!initialPadSubs);
  const [dubEnabled, setDubEnabled] = useState(!!dubDefaults);
  const [batchMode, setBatchMode] = useState(false);

  useEffect(() => {
    if (isOpen) {
      setOutputPath(defaultOutputPath);
      setDubEnabled(!!dubDefaults);
      setRenderSpeed(initialRenderSpeed ?? 'balanced');
      setPadSubs(!!initialPadSubs);
      setBatchMode(false);
    }
  }, [isOpen, defaultOutputPath, dubDefaults, initialRenderSpeed, initialPadSubs]);

  const allItems = useMemo(() => {
    const seen = new Set<string>();
    const list: { videoPath: string; segments: SubtitleSegment[] }[] = [];
    if (segments.length > 0 && videoPath) {
      seen.add(videoPath);
      list.push({ videoPath, segments });
    }
    for (const b of batchItems ?? []) {
      if (!seen.has(b.videoPath) && b.segments.length > 0) {
        seen.add(b.videoPath);
        list.push(b);
      }
    }
    return list;
  }, [videoPath, segments, batchItems]);

  if (!isOpen) return null;

  const handleBrowseOutputPath = async () => {
    const ext = mode === 'hardsub' ? 'mp4' : mode === 'softsub' ? 'mkv' : 'srt';
    const filename = outputPath ? outputPath.split(/[/\\]/).pop() || `subtitled.${ext}` : `subtitled.${ext}`;
    const selected = await pickSaveFile(filename, ext);
    if (selected) {
      setOutputPath(selected);
    }
  };

  const baseName = (p: string) => p.split(/[/\\]/).pop() ?? p;
  const stemOf = (p: string) => baseName(p).replace(/\.[^.]+$/, '');
  const dirOf = (p: string) => {
    const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
    return i > 0 ? p.substring(0, i) : '.';
  };
  const extFor = (m: ExportMode) => (m === 'hardsub' ? 'mp4' : m === 'softsub' ? 'mkv' : 'srt');
  const batchOutPath = (vp: string) =>
    `${dirOf(outputPath)}/${stemOf(vp)}_subtitled.${extFor(mode)}`;

  // Items to export: batch selection, or the current video, or all items
  // when the current video has no segments (batch-only case).
  const items =
    batchMode && allItems.length > 1
      ? allItems
      : segments.length > 0
        ? [{ videoPath, segments }]
        : allItems;

  // Build one request per selected item and hand them to the app-level
  // export queue — the modal closes and rendering runs in the background.
  const handleStartExport = () => {
    const dubActive = dubEnabled && !!dubDefaults;

    const requests: ExportRequest[] = items.map((it) => ({
      video_path: it.videoPath,
      output_path: items.length > 1 ? batchOutPath(it.videoPath) : outputPath,
      mode,
      bilingual,
      style_preset: stylePreset,
      segments: it.segments,
      dub: dubActive ? { enabled: true, ...dubDefaults } : null,
      job_id: `export-${Date.now()}-${reqCounter++}`,
      render_speed: renderSpeed,
      pad_subs: mode === 'hardsub' && padSubs ? 0 : null,
    }));

    onStartExport(requests);
    onClose();
  };

  const startCount = items.length;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm p-4 select-none">
      <div className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-2xl shadow-2xl overflow-hidden flex flex-col text-xs text-slate-200">
        {/* Header */}
        <div className="px-6 py-4 border-b border-slate-800/80 flex items-center justify-between bg-slate-950/40">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400">
              <Film className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-white tracking-tight">Xuất Phụ Đề & Video Hoàn Thiện</h2>
              <p className="text-[11px] text-slate-400">Chọn định dạng và cấu hình xuất video hoặc file phụ đề</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 flex items-center justify-center text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 flex flex-col gap-5 overflow-y-auto max-h-[calc(85vh-130px)]">
          {/* 0. Batch selector — only when multiple exportable jobs exist */}
          {allItems.length > 1 && (
            <div className="flex rounded-lg border border-slate-700 overflow-hidden text-xs shrink-0">
              <button
                type="button"
                onClick={() => setBatchMode(false)}
                className={`flex-1 px-3 py-2 transition ${
                  !batchMode ? 'bg-blue-600/20 text-blue-300 font-semibold' : 'bg-slate-900/60 text-slate-400 hover:bg-slate-800'
                }`}
              >
                Video hiện tại
              </button>
              <button
                type="button"
                onClick={() => setBatchMode(true)}
                className={`flex-1 px-3 py-2 transition ${
                  batchMode ? 'bg-blue-600/20 text-blue-300 font-semibold' : 'bg-slate-900/60 text-slate-400 hover:bg-slate-800'
                }`}
              >
                Tất cả {allItems.length} video
              </button>
            </div>
          )}

          {/* 1. Export Mode Selection */}
          <div className="flex flex-col gap-2.5">
            <label className="text-[11px] font-semibold uppercase tracking-wider text-slate-400">
              1. Chế độ xuất video & phụ đề
            </label>
            <div className="grid grid-cols-3 gap-3">
              {EXPORT_MODES.map((item) => {
                const Icon = item.icon;
                const isSelected = mode === item.id;
                return (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => setMode(item.id)}
                    className={`p-3.5 min-w-0 overflow-hidden rounded-xl border text-left flex flex-col gap-2 transition-all ${
                      isSelected
                        ? 'bg-blue-600/10 border-blue-500 text-white shadow-sm ring-1 ring-blue-500/30'
                        : 'bg-slate-900/60 border-slate-800 hover:border-slate-700 hover:bg-slate-850 text-slate-400'
                    }`}
                  >
                    {/* flex-wrap lets the badge drop below the title instead of truncating it */}
                    <div className="flex items-center gap-1.5 flex-wrap min-w-0">
                      <Icon className={`w-4 h-4 shrink-0 ${isSelected ? item.color : 'text-slate-400'}`} />
                      <span className={`font-semibold text-xs whitespace-nowrap ${isSelected ? 'text-white' : 'text-slate-300'}`}>
                        {item.title}
                      </span>
                      {item.badge && (
                        <span className="text-[9px] font-bold px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-300 border border-blue-500/30 shrink-0">
                          {item.badge}
                        </span>
                      )}
                    </div>
                    <p className="text-[11px] leading-relaxed text-slate-400 break-words">
                      {item.desc}
                    </p>
                  </button>
                );
              })}
            </div>
          </div>

          {/* 2. Styling & Bilingual options */}
          <div className="grid grid-cols-2 gap-4 p-4 rounded-xl bg-slate-950/40 border border-slate-800/80">
            <div className="flex flex-col gap-1.5">
              <label className="text-[11px] font-semibold text-slate-300">
                2. Kiểu dáng phụ đề
              </label>
              <select
                value={stylePreset}
                onChange={(e) => setStylePreset(e.target.value as StylePreset)}
                disabled={mode === 'subs_only'}
                className="h-9 px-3 rounded-lg border border-slate-700 bg-slate-900 text-slate-200 text-xs focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500/40 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
              >
                <option value="youtube_yellow">YouTube Yellow</option>
                <option value="classic_white">Classic White</option>
                <option value="netflix_modern">Netflix Modern</option>
                <option value="bilingual_highlight">Bilingual Highlight</option>
              </select>
              {mode === 'subs_only' && (
                <span className="text-[10px] text-slate-500">Kiểu dáng chỉ áp dụng cho Hardsub / Softsub</span>
              )}
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-[11px] font-semibold text-slate-300">
                Tốc độ render
              </label>
              <select
                aria-label="Tốc độ render"
                value={renderSpeed}
                onChange={(e) => setRenderSpeed(e.target.value as RenderSpeed)}
                disabled={mode !== 'hardsub'}
                className="h-9 px-3 rounded-lg border border-slate-700 bg-slate-900 text-slate-200 text-xs focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500/40 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
              >
                <option value="fast">Nhanh</option>
                <option value="balanced">Cân bằng</option>
                <option value="quality">Đẹp (chậm)</option>
              </select>
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-[11px] font-semibold text-slate-300">
                Sub nằm dưới video
              </label>
              <button
                type="button"
                disabled={mode !== 'hardsub'}
                onClick={() => setPadSubs(!padSubs)}
                className={`h-9 px-3 rounded-lg border flex items-center justify-between transition-colors disabled:opacity-40 disabled:cursor-not-allowed ${
                  padSubs && mode === 'hardsub'
                    ? 'border-blue-500/60 bg-blue-600/10 text-slate-200'
                    : 'border-slate-700 bg-slate-900 text-slate-400 hover:border-slate-600'
                }`}
              >
                <span className="text-xs">Không che hình</span>
                <span
                  className={`relative w-8 h-[18px] rounded-full transition-colors shrink-0 ${
                    padSubs && mode === 'hardsub' ? 'bg-blue-600' : 'bg-slate-700'
                  }`}
                >
                  <span
                    className={`absolute top-[2px] left-[2px] w-[14px] h-[14px] rounded-full bg-white shadow transition-transform ${
                      padSubs && mode === 'hardsub' ? 'translate-x-[14px]' : ''
                    }`}
                  />
                </span>
              </button>
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-[11px] font-semibold text-slate-300">
                Hiển thị song ngữ
              </label>
              <button
                type="button"
                onClick={() => setBilingual(!bilingual)}
                className={`h-9 px-3 rounded-lg border flex items-center justify-between transition-colors ${
                  bilingual
                    ? 'border-blue-500/60 bg-blue-600/10 text-slate-200'
                    : 'border-slate-700 bg-slate-900 text-slate-400 hover:border-slate-600'
                }`}
              >
                <span className="text-xs">Xuất cả gốc & dịch</span>
                <span
                  className={`relative w-8 h-[18px] rounded-full transition-colors shrink-0 ${
                    bilingual ? 'bg-blue-600' : 'bg-slate-700'
                  }`}
                >
                  <span
                    className={`absolute top-[2px] left-[2px] w-[14px] h-[14px] rounded-full bg-white shadow transition-transform ${
                      bilingual ? 'translate-x-[14px]' : ''
                    }`}
                  />
                </span>
              </button>
            </div>
          </div>

          {/* 2b. Voice dubbing toggle */}
          {dubDefaults && (
            <button
              type="button"
              onClick={() => setDubEnabled(!dubEnabled)}
              className={`w-full px-4 py-3 rounded-xl border flex items-center justify-between transition-colors ${
                dubEnabled
                  ? 'border-emerald-500/60 bg-emerald-600/10 text-slate-200'
                  : 'border-slate-800 bg-slate-950/40 text-slate-400 hover:border-slate-700'
              }`}
            >
              <span className="flex items-center gap-2.5">
                <Mic className={`w-4 h-4 ${dubEnabled ? 'text-emerald-400' : 'text-slate-500'}`} />
                <span className="flex flex-col items-start gap-0.5">
                  <span className="text-xs font-semibold">Lồng tiếng theo phụ đề đã dịch</span>
                  <span className="text-[10px] text-slate-500">
                    TTS {dubDefaults.engine === 'local' ? 'Piper (offline)' : 'API'} · giọng gốc nền{' '}
                    {Math.round(dubDefaults.original_volume * 100)}%
                  </span>
                </span>
              </span>
              <span
                className={`relative w-9 h-5 rounded-full transition-colors shrink-0 ${
                  dubEnabled ? 'bg-emerald-600' : 'bg-slate-700'
                }`}
              >
                <span
                  className={`absolute top-[3px] left-[3px] w-[14px] h-[14px] rounded-full bg-white shadow transition-transform ${
                    dubEnabled ? 'translate-x-[16px]' : ''
                  }`}
                />
              </span>
            </button>
          )}

          {/* 3. Output Destination */}
          <div className="flex flex-col gap-1.5">
            <label className="text-[11px] font-semibold uppercase tracking-wider text-slate-400">
              3. Vị trí lưu file hoàn thiện
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                value={outputPath}
                onChange={(e) => setOutputPath(e.target.value)}
                placeholder="/duong/dan/toi/file_xuat.mp4"
                className="flex-1 h-9 bg-slate-900 border border-slate-700 rounded-lg px-3 text-slate-200 font-mono text-[11px] focus:outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/40"
              />
              <button
                type="button"
                onClick={handleBrowseOutputPath}
                className="h-9 px-3.5 flex items-center gap-1.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 rounded-lg text-slate-200 text-xs font-medium transition shrink-0"
              >
                <Folder className="w-3.5 h-3.5 text-blue-400" />
                <span>Duyệt...</span>
              </button>
            </div>
            {batchMode && allItems.length > 1 && (
              <span className="text-[10px] text-slate-500">
                Mỗi video → <span className="font-mono">{'<tên>'}_subtitled.{extFor(mode)}</span> trong cùng thư mục trên
              </span>
            )}
          </div>
        </div>

        {/* Footer Actions */}
        <div className="px-6 py-4 bg-slate-950/40 border-t border-slate-800/80 flex items-center justify-between gap-3">
          <span className="text-[10px] text-slate-500">
            Xuất chạy nền — theo dõi ở danh sách xử lý & thanh tiến trình
          </span>
          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={onClose}
              className="h-9 px-4 rounded-lg text-slate-300 hover:text-white hover:bg-slate-800 transition text-xs font-medium"
            >
              Đóng
            </button>

            <button
              type="button"
              onClick={handleStartExport}
              disabled={startCount === 0}
              className="h-9 px-5 flex items-center gap-2 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-semibold rounded-lg shadow-md shadow-emerald-950 transition text-xs cursor-pointer disabled:cursor-not-allowed"
            >
              <Play className="w-3.5 h-3.5" />
              <span>{startCount > 1 ? `Bắt đầu xử lý (${startCount} video)` : 'Bắt đầu xử lý'}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
