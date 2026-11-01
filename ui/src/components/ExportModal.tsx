import React, { useState } from 'react';
import { X, Film, Layers, FileCode, FolderOpen, CheckCircle, Loader2, Folder } from 'lucide-react';
import type { ExportMode, StylePreset, ExportRequest, SubtitleSegment } from '../types';
import { pickSaveFile } from '../lib/ipc';

interface ExportModalProps {
  isOpen: boolean;
  onClose: () => void;
  videoPath: string;
  defaultOutputPath: string;
  segments: SubtitleSegment[];
  initialPreset: StylePreset;
  initialBilingual: boolean;
  onExport: (req: ExportRequest) => Promise<string>;
  onOpenFolder: (path: string) => void;
}

export const ExportModal: React.FC<ExportModalProps> = ({
  isOpen,
  onClose,
  videoPath,
  defaultOutputPath,
  segments,
  initialPreset,
  initialBilingual,
  onExport,
  onOpenFolder,
}) => {
  const [mode, setMode] = useState<ExportMode>('hardsub');
  const [outputPath, setOutputPath] = useState(defaultOutputPath);
  const [stylePreset, setStylePreset] = useState<StylePreset>(initialPreset);
  const [bilingual, setBilingual] = useState(initialBilingual);
  const [isExporting, setIsExporting] = useState(false);
  const [exportResult, setExportResult] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleBrowseOutputPath = async () => {
    const ext = mode === 'hardsub' ? 'mp4' : mode === 'softsub' ? 'mkv' : 'srt';
    const filename = outputPath ? outputPath.split('/').pop() || `subtitled.${ext}` : `subtitled.${ext}`;
    const selected = await pickSaveFile(filename, ext);
    if (selected) {
      setOutputPath(selected);
    }
  };

  const handleStartExport = async () => {
    setIsExporting(true);
    setExportResult(null);
    setErrorMsg(null);

    try {
      const result = await onExport({
        video_path: videoPath,
        output_path: outputPath,
        mode,
        bilingual,
        style_preset: stylePreset,
        segments,
      });
      setExportResult(result);
    } catch (err) {
      setErrorMsg(String(err));
    } finally {
      setIsExporting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 select-none">
      <div className="bg-slate-900 border border-slate-700/80 rounded-xl w-full max-w-lg shadow-2xl overflow-hidden flex flex-col text-xs text-slate-200">
        {/* Header */}
        <div className="px-5 py-4 border-b border-slate-800 flex items-center justify-between">
          <div className="flex items-center gap-2 text-sm font-bold text-white">
            <Film className="w-4 h-4 text-emerald-400" />
            <span>Xuất Phụ Đề / Video Hoàn Thiện</span>
          </div>
          <button
            onClick={onClose}
            disabled={isExporting}
            className="p-1 text-slate-400 hover:text-white rounded transition"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content */}
        <div className="p-5 flex flex-col gap-4">
          {/* Export Mode Selection */}
          <div className="flex flex-col gap-2">
            <label className="font-semibold text-slate-300">1. Chế độ xuất:</label>
            <div className="grid grid-cols-3 gap-2.5">
              {/* Hardsub */}
              <div
                onClick={() => setMode('hardsub')}
                className={`p-3 rounded-lg border cursor-pointer flex flex-col gap-1 transition ${
                  mode === 'hardsub'
                    ? 'bg-blue-950/50 border-blue-500 text-white shadow-sm'
                    : 'bg-slate-800/40 border-slate-700/60 hover:bg-slate-800 text-slate-400'
                }`}
              >
                <div className="flex items-center gap-1.5 font-bold">
                  <Film className="w-3.5 h-3.5 text-amber-400" />
                  <span>Hardsub</span>
                </div>
                <span className="text-[11px] leading-tight text-slate-400">
                  Gắn cứng phụ đề vào hình ảnh video.
                </span>
              </div>

              {/* Softsub */}
              <div
                onClick={() => setMode('softsub')}
                className={`p-3 rounded-lg border cursor-pointer flex flex-col gap-1 transition ${
                  mode === 'softsub'
                    ? 'bg-blue-950/50 border-blue-500 text-white shadow-sm'
                    : 'bg-slate-800/40 border-slate-700/60 hover:bg-slate-800 text-slate-400'
                }`}
              >
                <div className="flex items-center gap-1.5 font-bold">
                  <Layers className="w-3.5 h-3.5 text-indigo-400" />
                  <span>Softsub</span>
                </div>
                <span className="text-[11px] leading-tight text-slate-400">
                  Nhúng track phụ đề vào MP4/MKV (siêu nhanh).
                </span>
              </div>

              {/* Subs-only */}
              <div
                onClick={() => setMode('subs_only')}
                className={`p-3 rounded-lg border cursor-pointer flex flex-col gap-1 transition ${
                  mode === 'subs_only'
                    ? 'bg-blue-950/50 border-blue-500 text-white shadow-sm'
                    : 'bg-slate-800/40 border-slate-700/60 hover:bg-slate-800 text-slate-400'
                }`}
              >
                <div className="flex items-center gap-1.5 font-bold">
                  <FileCode className="w-3.5 h-3.5 text-cyan-400" />
                  <span>Subs-only</span>
                </div>
                <span className="text-[11px] leading-tight text-slate-400">
                  Chỉ xuất file .srt và .ass độc lập.
                </span>
              </div>
            </div>
          </div>

          {/* Subtitle Style & Bilingual */}
          <div className="grid grid-cols-2 gap-3 pt-2 border-t border-slate-800">
            <div className="flex flex-col gap-1.5">
              <label className="font-semibold text-slate-300">2. Kiểu dáng phụ đề:</label>
              <select
                value={stylePreset}
                onChange={(e) => setStylePreset(e.target.value as StylePreset)}
                className="bg-slate-800 border border-slate-700 rounded px-2.5 py-1.5 text-slate-200 focus:outline-none"
              >
                <option value="youtube_yellow">YouTube Yellow</option>
                <option value="classic_white">Classic White</option>
                <option value="netflix_modern">Netflix Modern</option>
                <option value="bilingual_highlight">Bilingual Highlight</option>
              </select>
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="font-semibold text-slate-300">Song ngữ:</label>
              <label className="flex items-center gap-2 mt-1.5 cursor-pointer text-slate-300">
                <input
                  type="checkbox"
                  checked={bilingual}
                  onChange={(e) => setBilingual(e.target.checked)}
                  className="rounded bg-slate-800 border-slate-700 text-blue-500 focus:ring-0"
                />
                <span>Xuất cả tiếng gốc và bản dịch</span>
              </label>
            </div>
          </div>

          {/* Output Path Input */}
          <div className="flex flex-col gap-1.5 pt-2 border-t border-slate-800">
            <label className="font-semibold text-slate-300">3. Vị trí lưu file:</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={outputPath}
                onChange={(e) => setOutputPath(e.target.value)}
                className="flex-1 bg-slate-800 border border-slate-700 rounded px-3 py-1.5 text-slate-200 font-mono text-[11px] focus:outline-none focus:border-blue-500"
              />
              <button
                type="button"
                onClick={handleBrowseOutputPath}
                className="flex items-center gap-1.5 bg-slate-800 hover:bg-slate-700 border border-slate-700 px-3 py-1.5 rounded text-[11px] text-slate-300 transition shrink-0"
              >
                <Folder className="w-3.5 h-3.5 text-blue-400" />
                <span>Duyệt...</span>
              </button>
            </div>
          </div>

          {/* Result or Error Banner */}
          {exportResult && (
            <div className="bg-emerald-950/50 border border-emerald-800/80 rounded-lg p-3 text-emerald-300 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <CheckCircle className="w-4 h-4 shrink-0" />
                <span>{exportResult}</span>
              </div>
              <button
                onClick={() => onOpenFolder(outputPath)}
                className="flex items-center gap-1 bg-emerald-800/60 hover:bg-emerald-800 text-white px-2 py-1 rounded text-[11px] transition"
              >
                <FolderOpen className="w-3 h-3" />
                <span>Mở thư mục</span>
              </button>
            </div>
          )}

          {errorMsg && (
            <div className="bg-rose-950/50 border border-rose-800/80 rounded-lg p-3 text-rose-300">
              {errorMsg}
            </div>
          )}
        </div>

        {/* Footer Actions */}
        <div className="px-5 py-3.5 bg-slate-800/40 border-t border-slate-800 flex items-center justify-end gap-2.5">
          <button
            onClick={onClose}
            disabled={isExporting}
            className="px-4 py-1.5 rounded text-slate-300 hover:text-white hover:bg-slate-800 transition"
          >
            Đóng
          </button>

          <button
            onClick={handleStartExport}
            disabled={isExporting || segments.length === 0}
            className="flex items-center gap-1.5 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-semibold px-5 py-1.5 rounded shadow transition"
          >
            {isExporting ? (
              <>
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                <span>Đang render...</span>
              </>
            ) : (
              <span>Render & Xuất Ngay</span>
            )}
          </button>
        </div>
      </div>
    </div>
  );
};
