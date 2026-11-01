import React from 'react';
import {
  FileVideo,
  Play,
  Square,
  Cpu,
  Zap,
  Download,
  Languages,
  Sparkles,
  Palette,
  Settings,
} from 'lucide-react';
import type { SystemInfo, StylePreset, ProfileView } from '../types';
import { KIND_ICONS } from '../lib/providers';

interface HeaderBarProps {
  videoPath: string;
  jobCount?: number;
  onSelectVideo: () => void;
  whisperModel: string;
  onSelectWhisperModel: (m: string) => void;
  profiles: ProfileView[];
  defaultProfileId: string | null;
  onSelectProfile: (id: string) => void;
  onOpenSettings: () => void;
  targetLang: string;
  onSelectTargetLang: (l: string) => void;
  bilingual: boolean;
  onToggleBilingual: () => void;
  stylePreset: StylePreset;
  onSelectPreset: (p: StylePreset) => void;
  isRunning: boolean;
  onStart: () => void;
  onCancel: () => void;
  systemInfo: SystemInfo | null;
  onDownloadModel: (name: string) => void;
  downloadingModel: string | null;
}

export const HeaderBar: React.FC<HeaderBarProps> = ({
  videoPath,
  jobCount = 0,
  onSelectVideo,
  whisperModel,
  onSelectWhisperModel,
  profiles,
  defaultProfileId,
  onSelectProfile,
  onOpenSettings,
  targetLang,
  onSelectTargetLang,
  bilingual,
  onToggleBilingual,
  stylePreset,
  onSelectPreset,
  isRunning,
  onStart,
  onCancel,
  systemInfo,
  onDownloadModel,
  downloadingModel,
}) => {
  const currentWhisperInfo = systemInfo?.whisper_models.find((m) => m.name === whisperModel);
  const isDownloaded = currentWhisperInfo?.downloaded ?? true;
  const fileName = videoPath
    ? videoPath.split(/[/\\]/).pop()
    : jobCount > 0
      ? `${jobCount} file`
      : 'Chọn video...';

  return (
    <header className="h-16 bg-slate-800/90 backdrop-blur border-b border-slate-700/80 px-4 flex items-center justify-between gap-4 text-xs select-none z-20">
      {/* Brand & File Picker */}
      <div className="flex items-center gap-3 min-w-0">
        <div className="flex items-center gap-1.5 font-bold text-sm tracking-wide bg-gradient-to-r from-blue-400 to-indigo-400 bg-clip-text text-transparent">
          <Sparkles className="w-4 h-4 text-blue-400" />
          <span>ECHO</span>
        </div>

        <button
          onClick={onSelectVideo}
          disabled={isRunning}
          className="flex items-center gap-2 bg-slate-700/60 hover:bg-slate-700 text-slate-200 px-3 py-1.5 rounded-md border border-slate-600/70 transition-colors max-w-xs truncate"
          title={videoPath || 'Chọn file video (.mp4, .mkv, .mov)'}
        >
          <FileVideo className="w-3.5 h-3.5 text-blue-400 shrink-0" />
          <span className="truncate font-medium">{fileName}</span>
        </button>
      </div>

      {/* Center Config Controls */}
      <div className="flex items-center gap-2.5">
        {/* Hardware badge */}
        <div
          className={`flex items-center gap-1 px-2 py-1 rounded text-[11px] font-medium border ${
            systemInfo?.has_cuda
              ? 'bg-emerald-950/40 text-emerald-400 border-emerald-800/50'
              : 'bg-slate-700/40 text-slate-400 border-slate-600/40'
          }`}
          title={systemInfo?.has_cuda ? 'GPU NVIDIA CUDA kích hoạt' : 'Chế độ CPU đa luồng'}
        >
          {systemInfo?.has_cuda ? (
            <>
              <Zap className="w-3 h-3 text-emerald-400" />
              <span>CUDA GPU</span>
            </>
          ) : (
            <>
              <Cpu className="w-3 h-3" />
              <span>CPU Mode</span>
            </>
          )}
        </div>

        {/* Whisper selector */}
        <div className="flex items-center bg-slate-900/60 rounded-md border border-slate-700 px-1.5 py-0.5">
          <span className="text-slate-400 mr-1.5">🎙️</span>
          <select
            value={whisperModel}
            onChange={(e) => onSelectWhisperModel(e.target.value)}
            disabled={isRunning}
            className="bg-transparent text-slate-200 focus:outline-none py-1 text-xs cursor-pointer"
          >
            {systemInfo?.whisper_models.map((m) => (
              <option key={m.name} value={m.name} className="bg-slate-800">
                {m.name} ({m.size_mb}MB){m.downloaded ? ' ✓' : ''}
              </option>
            )) || (
              <option value="base" className="bg-slate-800">
                base
              </option>
            )}
          </select>

          {!isDownloaded && (
            <button
              onClick={() => onDownloadModel(whisperModel)}
              disabled={downloadingModel === whisperModel || isRunning}
              className="ml-1 text-[11px] flex items-center gap-1 bg-blue-600 hover:bg-blue-500 text-white px-2 py-0.5 rounded transition"
              title="Tải model Whisper về máy"
            >
              <Download className="w-3 h-3" />
              <span>{downloadingModel === whisperModel ? 'Đang tải...' : 'Tải'}</span>
            </button>
          )}
        </div>

        {/* Provider profile selector */}
        <div className="flex items-center bg-slate-900/60 rounded-md border border-slate-700 px-1.5 py-0.5">
          <Languages className="w-3.5 h-3.5 text-indigo-400 mr-1.5" />
          <select
            aria-label="Provider profile"
            value={defaultProfileId ?? ''}
            onChange={(e) => onSelectProfile(e.target.value)}
            disabled={isRunning || profiles.length === 0}
            className="bg-transparent text-slate-200 focus:outline-none py-1 text-xs cursor-pointer max-w-[180px] truncate"
          >
            {profiles.length === 0 && (
              <option value="" className="bg-slate-800">
                Chưa có profile
              </option>
            )}
            {profiles.map((p) => (
              <option key={p.id} value={p.id} className="bg-slate-800">
                {KIND_ICONS[p.kind]} {p.name} · {p.model}
              </option>
            ))}
          </select>
          <button
            onClick={onOpenSettings}
            aria-label="Cài đặt"
            title="Cài đặt provider & chung"
            className="ml-1 text-slate-400 hover:text-white p-0.5"
          >
            <Settings className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Target Language */}
        <select
          value={targetLang}
          onChange={(e) => onSelectTargetLang(e.target.value)}
          disabled={isRunning}
          className="bg-slate-900/60 text-slate-200 border border-slate-700 rounded-md px-2 py-1 text-xs focus:outline-none cursor-pointer"
        >
          <option value="vi" className="bg-slate-800">Tiếng Việt (VI)</option>
          <option value="en" className="bg-slate-800">English (EN)</option>
          <option value="ja" className="bg-slate-800">日本語 (JA)</option>
          <option value="ko" className="bg-slate-800">한국어 (KO)</option>
          <option value="zh" className="bg-slate-800">中文 (ZH)</option>
        </select>

        {/* Subtitle Style Preset */}
        <div className="flex items-center bg-slate-900/60 rounded-md border border-slate-700 px-1.5 py-0.5">
          <Palette className="w-3.5 h-3.5 text-amber-400 mr-1.5" />
          <select
            value={stylePreset}
            onChange={(e) => onSelectPreset(e.target.value as StylePreset)}
            disabled={isRunning}
            className="bg-transparent text-slate-200 focus:outline-none py-1 text-xs cursor-pointer"
          >
            <option value="youtube_yellow" className="bg-slate-800">YouTube Yellow</option>
            <option value="classic_white" className="bg-slate-800">Classic White</option>
            <option value="netflix_modern" className="bg-slate-800">Netflix Modern</option>
            <option value="bilingual_highlight" className="bg-slate-800">Bilingual Highlight</option>
          </select>
        </div>

        {/* Bilingual Toggle */}
        <button
          onClick={onToggleBilingual}
          disabled={isRunning}
          className={`flex items-center gap-1 px-2.5 py-1 rounded-md border font-medium transition-colors ${
            bilingual
              ? 'bg-blue-600/30 text-blue-300 border-blue-500/60'
              : 'bg-slate-800 text-slate-400 border-slate-700'
          }`}
          title="Bật/Tắt chế độ phụ đề song ngữ (cả tiếng gốc và bản dịch)"
        >
          <span>Song ngữ</span>
          <span
            className={`w-2 h-2 rounded-full ${
              bilingual ? 'bg-blue-400' : 'bg-slate-600'
            }`}
          />
        </button>
      </div>

      {/* Start / Cancel Action Button */}
      <div className="flex items-center">
        {isRunning ? (
          <button
            onClick={onCancel}
            className="flex items-center gap-1.5 bg-rose-600 hover:bg-rose-500 text-white font-semibold px-4 py-1.5 rounded-md shadow transition"
          >
            <Square className="w-3.5 h-3.5 fill-current" />
            <span>Hủy tác vụ</span>
          </button>
        ) : (
          <button
            onClick={onStart}
            disabled={!videoPath && jobCount === 0}
            className={`flex items-center gap-1.5 font-semibold px-4 py-1.5 rounded-md shadow transition ${
              videoPath || jobCount > 0
                ? 'bg-blue-600 hover:bg-blue-500 text-white cursor-pointer'
                : 'bg-slate-700 text-slate-500 cursor-not-allowed'
            }`}
          >
            <Play className="w-3.5 h-3.5 fill-current" />
            <span>Bắt Đầu</span>
          </button>
        )}
      </div>
    </header>
  );
};
