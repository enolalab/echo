import React, { useState, useEffect } from 'react';
import {
  X,
  Film,
  Layers,
  FileCode,
  FolderOpen,
  CheckCircle2,
  Loader2,
  Folder,
  Play,
  Copy,
  Check,
  AlertCircle,
  Mic,
  ChevronRight,
} from 'lucide-react';
import type {
  ExportMode,
  StylePreset,
  ExportRequest,
  SubtitleSegment,
  DubOptions,
  ExportProgress,
} from '../types';
import { pickSaveFile } from '../lib/ipc';
import { listen } from '@tauri-apps/api/event';

interface ExportModalProps {
  isOpen: boolean;
  onClose: () => void;
  videoPath: string;
  defaultOutputPath: string;
  segments: SubtitleSegment[];
  initialPreset: StylePreset;
  initialBilingual: boolean;
  dubDefaults?: Omit<DubOptions, 'enabled'>;
  onExport: (req: ExportRequest) => Promise<string>;
  onOpenFolder: (path: string) => void;
  onExported?: (outputPath: string | null) => void;
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
  dubDefaults,
  onExport,
  onOpenFolder,
  onExported,
}) => {
  const [mode, setMode] = useState<ExportMode>('hardsub');
  const [outputPath, setOutputPath] = useState(defaultOutputPath);
  const [stylePreset, setStylePreset] = useState<StylePreset>(initialPreset);
  const [bilingual, setBilingual] = useState(initialBilingual);
  const [dubEnabled, setDubEnabled] = useState(!!dubDefaults);
  const [exportStage, setExportStage] = useState<'idle' | 'dubbing' | 'rendering'>('idle');
  const [dubCount, setDubCount] = useState<{ index: number; total: number } | null>(null);
  const [isExporting, setIsExporting] = useState(false);
  const [exportResult, setExportResult] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (isOpen) {
      setOutputPath(defaultOutputPath);
      setExportResult(null);
      setErrorMsg(null);
      setCopied(false);
      setExportStage('idle');
      setDubCount(null);
      setDubEnabled(!!dubDefaults);
    }
  }, [isOpen, defaultOutputPath, dubDefaults]);

  if (!isOpen) return null;

  const handleBrowseOutputPath = async () => {
    const ext = mode === 'hardsub' ? 'mp4' : mode === 'softsub' ? 'mkv' : 'srt';
    const filename = outputPath ? outputPath.split(/[/\\]/).pop() || `subtitled.${ext}` : `subtitled.${ext}`;
    const selected = await pickSaveFile(filename, ext);
    if (selected) {
      setOutputPath(selected);
    }
  };

  const handleStartExport = async () => {
    setIsExporting(true);
    setExportResult(null);
    setErrorMsg(null);
    setExportStage(dubEnabled ? 'dubbing' : 'rendering');
    setDubCount(null);

    const jobId = `export-${Date.now()}`;
    // Fire-and-forget: don't block onExport on listener registration —
    // outside Tauri (browser/test) this promise may never settle.
    const unlistenPromise = listen<ExportProgress>('export-progress', (e) => {
      const p = e.payload;
      if (p.job_id !== jobId) return;
      if (p.stage === 'dubbing') {
        setExportStage('dubbing');
        setDubCount({ index: p.index, total: p.total });
      } else if (p.stage === 'rendering') {
        setExportStage('rendering');
      }
    }).catch(() => null);

    try {
      const result = await onExport({
        video_path: videoPath,
        output_path: outputPath,
        mode,
        bilingual,
        style_preset: stylePreset,
        segments,
        dub: dubEnabled && dubDefaults ? { enabled: true, ...dubDefaults } : null,
        job_id: jobId,
      });
      setExportResult(result);
      onExported?.(mode === 'subs_only' ? null : outputPath);
    } catch (err) {
      setErrorMsg(String(err));
    } finally {
      void unlistenPromise.then((u) => u?.());
      setExportStage('idle');
      setDubCount(null);
      setIsExporting(false);
    }
  };

  const handleCopyPath = () => {
    if (navigator.clipboard?.writeText) {
      navigator.clipboard.writeText(outputPath).then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      });
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm p-4 select-none">
      <div className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-xl shadow-2xl overflow-hidden flex flex-col text-xs text-slate-200">
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
            disabled={isExporting}
            className="w-8 h-8 flex items-center justify-center text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition disabled:opacity-40"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 flex flex-col gap-5 overflow-y-auto max-h-[calc(85vh-130px)]">
          {/* Stage progress card — mirrors ProgressDrawer styling */}
          {isExporting && (
            <div
              data-testid="export-progress"
              className="rounded-xl border border-slate-800 bg-slate-950/40 p-4 flex flex-col gap-3"
            >
              {/* Stage breadcrumb */}
              <div className="flex items-center gap-1.5">
                {dubEnabled && (
                  <>
                    <span
                      className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-medium border ${
                        exportStage === 'rendering'
                          ? 'bg-emerald-950/40 text-emerald-400 border-emerald-800/50'
                          : 'bg-blue-950/60 text-blue-300 border-blue-500/80 animate-pulse'
                      }`}
                    >
                      {exportStage === 'rendering' ? (
                        <CheckCircle2 className="w-3 h-3" />
                      ) : (
                        <Loader2 className="w-3 h-3 animate-spin" />
                      )}
                      Lồng tiếng
                    </span>
                    <ChevronRight className="w-3 h-3 text-slate-700" />
                  </>
                )}
                <span
                  className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-medium border ${
                    exportStage === 'rendering'
                      ? 'bg-blue-950/60 text-blue-300 border-blue-500/80 animate-pulse'
                      : 'bg-slate-800/40 text-slate-500 border-slate-700/40'
                  }`}
                >
                  {exportStage === 'rendering' ? (
                    <Loader2 className="w-3 h-3 animate-spin" />
                  ) : (
                    <span className="w-1.5 h-1.5 rounded-full bg-slate-600" />
                  )}
                  {mode === 'subs_only' ? 'Xuất phụ đề' : 'Render video'}
                </span>
              </div>

              {/* Progress bar */}
              <div className="w-full bg-slate-800/60 h-1.5 rounded-full overflow-hidden">
                <div
                  data-testid="export-progress-bar"
                  className={`h-full transition-all duration-300 ease-out ${
                    exportStage === 'rendering'
                      ? 'w-full bg-gradient-to-r from-blue-500 via-indigo-500 to-emerald-400 animate-pulse'
                      : 'bg-gradient-to-r from-blue-500 via-indigo-500 to-emerald-400'
                  }`}
                  style={
                    exportStage === 'dubbing'
                      ? { width: `${dubCount && dubCount.total > 0 ? Math.min(100, Math.round((dubCount.index / dubCount.total) * 100)) : 2}%` }
                      : undefined
                  }
                />
              </div>

              {/* Status line */}
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-slate-300">
                  {exportStage === 'dubbing' && dubCount
                    ? `Đang lồng tiếng ${dubCount.index}/${dubCount.total}...`
                    : exportStage === 'dubbing'
                      ? 'Đang khởi tạo lồng tiếng...'
                      : mode === 'subs_only'
                        ? 'Đang ghi file phụ đề...'
                        : 'Đang render video...'}
                </span>
                {exportStage === 'dubbing' && dubCount && dubCount.total > 0 && (
                  <span className="bg-blue-950/80 text-blue-400 font-bold px-1.5 py-0.5 rounded border border-blue-800/50">
                    {Math.round((dubCount.index / dubCount.total) * 100)}%
                  </span>
                )}
              </div>
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
                    className={`relative p-3.5 rounded-xl border text-left flex flex-col gap-2 transition-all ${
                      isSelected
                        ? 'bg-blue-600/10 border-blue-500 text-white shadow-sm ring-1 ring-blue-500/30'
                        : 'bg-slate-900/60 border-slate-800 hover:border-slate-700 hover:bg-slate-850 text-slate-400'
                    }`}
                  >
                    {item.badge && (
                      <span className="absolute top-2.5 right-2.5 text-[9px] font-bold px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-300 border border-blue-500/30">
                        {item.badge}
                      </span>
                    )}
                    <div className="flex items-center gap-2">
                      <Icon className={`w-4 h-4 ${isSelected ? item.color : 'text-slate-400'}`} />
                      <span className={`font-semibold text-xs ${isSelected ? 'text-white' : 'text-slate-300'}`}>
                        {item.title}
                      </span>
                    </div>
                    <p className="text-[11px] leading-relaxed text-slate-400 pr-1">
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
                className="h-9 px-3 rounded-lg border border-slate-700 bg-slate-900 text-slate-200 text-xs focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500/40 cursor-pointer disabled:opacity-40"
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
          </div>

          {/* Result Banner with clean multi-row layout */}
          {exportResult && (
            <div className="rounded-xl border border-emerald-800/80 bg-emerald-950/40 p-4 flex flex-col gap-3">
              <div className="flex items-center justify-between gap-3 flex-wrap">
                <div className="flex items-center gap-2 text-emerald-400 font-semibold text-xs">
                  <CheckCircle2 className="w-4 h-4 shrink-0" />
                  <span>Xuất file thành công!</span>
                </div>
                <div className="flex items-center gap-2">
                  {mode !== 'subs_only' && (
                    <button
                      type="button"
                      onClick={onClose}
                      className="h-7 px-2.5 flex items-center gap-1.5 rounded-md bg-blue-600 hover:bg-blue-500 text-white font-medium text-[11px] shadow transition"
                    >
                      <Play className="w-3 h-3 fill-current" />
                      <span>Xem kết quả</span>
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => onOpenFolder(outputPath)}
                    className="h-7 px-2.5 flex items-center gap-1.5 rounded-md bg-emerald-700 hover:bg-emerald-600 text-white font-medium text-[11px] shadow transition"
                  >
                    <FolderOpen className="w-3.5 h-3.5" />
                    <span>Mở thư mục</span>
                  </button>
                </div>
              </div>

              {/* Path display with copy button */}
              <div className="flex items-center gap-2 bg-slate-950/60 border border-emerald-900/40 rounded-lg px-3 py-2 text-[11px] font-mono text-emerald-300">
                <span className="flex-1 truncate select-all" title={outputPath}>
                  {outputPath}
                </span>
                <button
                  type="button"
                  onClick={handleCopyPath}
                  className="p-1 text-slate-400 hover:text-white transition rounded shrink-0"
                  title="Copy đường dẫn"
                >
                  {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                </button>
              </div>
            </div>
          )}

          {/* Error Banner */}
          {errorMsg && (
            <div className="rounded-xl border border-rose-800/80 bg-rose-950/40 p-4 flex items-start gap-2.5 text-rose-300 text-xs">
              <AlertCircle className="w-4 h-4 text-rose-400 shrink-0 mt-0.5" />
              <div className="flex-1 leading-relaxed break-all">
                {errorMsg}
              </div>
            </div>
          )}
        </div>

        {/* Footer Actions */}
        <div className="px-6 py-4 bg-slate-950/40 border-t border-slate-800/80 flex items-center justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            disabled={isExporting}
            className="h-9 px-4 rounded-lg text-slate-300 hover:text-white hover:bg-slate-800 transition text-xs font-medium"
          >
            Đóng
          </button>

          <button
            type="button"
            onClick={handleStartExport}
            disabled={isExporting || segments.length === 0}
            className="h-9 px-5 flex items-center gap-2 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-semibold rounded-lg shadow-md shadow-emerald-950 transition text-xs cursor-pointer disabled:cursor-not-allowed"
          >
            {isExporting ? (
              <>
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                <span>Đang render...</span>
              </>
            ) : (
              <>
                <Film className="w-3.5 h-3.5" />
                <span>Render & Xuất Ngay</span>
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
};
