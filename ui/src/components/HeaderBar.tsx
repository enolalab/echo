import React from 'react';
import {
  FileVideo,
  Play,
  Square,
  Cpu,
  Zap,
  Download,
  Loader2,
  Languages,
  Palette,
  Settings,
  GraduationCap,
  Mic,
  Globe,
  ChevronDown,
  Clapperboard,
} from 'lucide-react';
import type { SystemInfo, StylePreset, ProfileView, Skill } from '../types';
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
  presets: Skill[];
  userSkills: Skill[];
  defaultSkillId: string | null;
  onSelectSkill: (id: string | null) => void;
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

const LANG_LABELS: Record<string, string> = {
  vi: 'Tiếng Việt',
  en: 'English',
  ja: '日本語',
  ko: '한국어',
  zh: '中文',
};

const PRESET_LABELS: Record<StylePreset, string> = {
  youtube_yellow: 'YouTube Yellow',
  classic_white: 'Classic White',
  netflix_modern: 'Netflix Modern',
  bilingual_highlight: 'Bilingual Highlight',
};

interface SelectChipProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  icon: React.ReactNode;
  label: string;
  /** Text shown in the chip; truncated by us (WebKitGTK ignores text-overflow on <select>). */
  display: string;
  /** Optional element rendered on the right, above the invisible select (e.g. a button). */
  trailing?: React.ReactNode;
}

/**
 * A settings "chip": icon + small label + current value. The native <select> is
 * stretched invisibly over the chip so the OS dropdown still opens on click,
 * while layout and truncation stay fully under our control.
 */
const SelectChip: React.FC<SelectChipProps> = ({
  icon,
  label,
  display,
  trailing,
  disabled,
  children,
  ...selectProps
}) => (
  <div
    className={`relative min-w-0 h-10 flex items-center gap-2.5 pl-3 pr-2.5 rounded-lg border bg-slate-900 transition-colors focus-within:border-blue-500 focus-within:ring-1 focus-within:ring-blue-500/40 ${
      disabled ? 'opacity-50 border-slate-800' : 'border-slate-700/80 hover:border-slate-500 hover:bg-slate-800/60'
    }`}
    title={`${label}: ${display}`}
  >
    <span className="shrink-0 flex">{icon}</span>
    <span className="min-w-0 flex-1 flex flex-col leading-tight">
      <span className="text-[10px] font-semibold uppercase tracking-wider text-slate-500 truncate">
        {label}
      </span>
      <span className="text-xs font-medium text-slate-100 truncate">{display}</span>
    </span>
    {trailing}
    <ChevronDown className="w-3.5 h-3.5 text-slate-500 shrink-0" />
    <select
      {...selectProps}
      disabled={disabled}
      className="absolute inset-0 w-full h-full opacity-0 cursor-pointer disabled:cursor-not-allowed"
    >
      {children}
    </select>
  </div>
);

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
  presets,
  userSkills,
  defaultSkillId,
  onSelectSkill,
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
  const hasInput = Boolean(videoPath) || jobCount > 0;
  const currentProfile = profiles.find((p) => p.id === defaultProfileId);
  const currentSkill = [...presets, ...userSkills].find((s) => s.id === defaultSkillId);
  const fileName = videoPath
    ? videoPath.split(/[/\\]/).pop()
    : jobCount > 0
      ? `${jobCount} file`
      : 'Mở video...';

  return (
    <header className="bg-slate-900 border-b border-slate-800 text-xs select-none z-20 shrink-0">
      {/* Row 1: brand · file · system · primary action */}
      <div className="h-12 px-4 flex items-center gap-3">
        <div className="flex items-center gap-2 shrink-0">
          <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shadow-md shadow-blue-900/40">
            <Clapperboard className="w-4 h-4 text-white" />
          </div>
          <span className="font-semibold text-sm text-slate-100 tracking-tight whitespace-nowrap">
            Echo Studio
          </span>
        </div>

        <div className="w-px h-6 bg-slate-800 shrink-0" />

        <button
          onClick={onSelectVideo}
          disabled={isRunning}
          className={`flex items-center gap-2 h-8 px-3 rounded-md border transition-colors min-w-0 max-w-sm disabled:opacity-50 disabled:cursor-not-allowed ${
            hasInput
              ? 'bg-slate-800 hover:bg-slate-700 border-slate-700 text-slate-200'
              : 'bg-blue-600/15 hover:bg-blue-600/25 border-blue-500/50 text-blue-200'
          }`}
          title={videoPath || 'Chọn file video (.mp4, .mkv, .mov)'}
        >
          <FileVideo className="w-4 h-4 text-blue-400 shrink-0" />
          <span className="truncate font-medium">{fileName}</span>
        </button>

        <div className="flex-1" />

        <div
          className={`flex items-center gap-1.5 h-7 px-2.5 rounded-full text-[11px] font-medium border shrink-0 whitespace-nowrap ${
            systemInfo?.has_cuda
              ? 'bg-emerald-950/40 text-emerald-400 border-emerald-800/50'
              : 'bg-slate-800/60 text-slate-400 border-slate-700'
          }`}
          title={systemInfo?.has_cuda ? 'GPU NVIDIA CUDA kích hoạt' : 'Chế độ CPU đa luồng'}
        >
          {systemInfo?.has_cuda ? <Zap className="w-3 h-3" /> : <Cpu className="w-3 h-3" />}
          <span>{systemInfo?.has_cuda ? 'CUDA GPU' : 'CPU'}</span>
        </div>

        <button
          onClick={onOpenSettings}
          aria-label="Cài đặt"
          title="Cài đặt provider & chung"
          className="w-8 h-8 flex items-center justify-center rounded-md text-slate-400 hover:text-white hover:bg-slate-800 transition-colors shrink-0"
        >
          <Settings className="w-4 h-4" />
        </button>

        {isRunning ? (
          <button
            onClick={onCancel}
            className="flex items-center gap-2 h-8 px-4 bg-rose-600 hover:bg-rose-500 text-white font-semibold rounded-md shadow transition shrink-0"
          >
            <Square className="w-3.5 h-3.5 fill-current" />
            <span>Hủy tác vụ</span>
          </button>
        ) : (
          <button
            onClick={onStart}
            disabled={!hasInput}
            title={hasInput ? 'Bắt đầu tạo phụ đề' : 'Hãy chọn video trước'}
            className={`flex items-center gap-2 h-8 px-4 font-semibold rounded-md transition shrink-0 ${
              hasInput
                ? 'bg-blue-600 hover:bg-blue-500 text-white shadow-md shadow-blue-900/40 cursor-pointer'
                : 'bg-slate-800 text-slate-500 border border-slate-700 cursor-not-allowed'
            }`}
          >
            <Play className="w-3.5 h-3.5 fill-current" />
            <span>Bắt Đầu</span>
          </button>
        )}
      </div>

      {/* Row 2: pipeline configuration as equal-height chips on a grid (never overflows) */}
      <div className="px-4 py-2 border-t border-slate-800/80 bg-slate-950/40 grid items-center gap-2 grid-cols-[minmax(0,1fr)_minmax(0,1.35fr)_minmax(0,1fr)_minmax(0,0.85fr)_minmax(0,1fr)_auto]">
        <SelectChip
          aria-label="Whisper model"
          icon={<Mic className="w-4 h-4 text-sky-400" />}
          label="Nhận dạng"
          display={
            currentWhisperInfo
              ? `${currentWhisperInfo.name} · ${isDownloaded ? `${currentWhisperInfo.size_mb}MB` : 'chưa tải'}`
              : whisperModel
          }
          value={whisperModel}
          onChange={(e) => onSelectWhisperModel(e.target.value)}
          disabled={isRunning}
          trailing={
            !isDownloaded && (
              <button
                onClick={() => onDownloadModel(whisperModel)}
                disabled={downloadingModel === whisperModel || isRunning}
                aria-label="Tải model"
                className="relative z-10 shrink-0 w-6 h-6 flex items-center justify-center bg-blue-600 hover:bg-blue-500 disabled:opacity-60 text-white rounded-md transition"
                title="Model chưa có trên máy — bấm để tải"
              >
                {downloadingModel === whisperModel ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Download className="w-3.5 h-3.5" />
                )}
              </button>
            )
          }
        >
          {systemInfo?.whisper_models.map((m) => (
            <option key={m.name} value={m.name}>
              {m.name} ({m.size_mb}MB){m.downloaded ? ' ✓' : ''}
            </option>
          )) || <option value="base">base</option>}
        </SelectChip>

        <SelectChip
          aria-label="Provider profile"
          icon={<Languages className="w-4 h-4 text-indigo-400" />}
          label="Dịch bằng"
          display={currentProfile ? `${currentProfile.name} · ${currentProfile.model}` : 'Chưa có profile'}
          value={defaultProfileId ?? ''}
          onChange={(e) => onSelectProfile(e.target.value)}
          disabled={isRunning || profiles.length === 0}
        >
          {profiles.length === 0 && <option value="">Chưa có profile</option>}
          {profiles.map((p) => (
            <option key={p.id} value={p.id}>
              {KIND_ICONS[p.kind]} {p.name} · {p.model}
            </option>
          ))}
        </SelectChip>

        <SelectChip
          aria-label="Skill"
          icon={<GraduationCap className="w-4 h-4 text-emerald-400" />}
          label="Skill"
          display={currentSkill?.name ?? 'Không dùng'}
          value={defaultSkillId ?? 'none'}
          onChange={(e) => onSelectSkill(e.target.value === 'none' ? null : e.target.value)}
          disabled={isRunning}
        >
          <option value="none">Không dùng</option>
          {presets.length > 0 && (
            <optgroup label="Có sẵn">
              {presets.map((s) => (
                <option key={s.id} value={s.id}>🔒 {s.name}</option>
              ))}
            </optgroup>
          )}
          {userSkills.length > 0 && (
            <optgroup label="Của tôi">
              {userSkills.map((s) => (
                <option key={s.id} value={s.id}>{s.name}</option>
              ))}
            </optgroup>
          )}
        </SelectChip>

        <SelectChip
          aria-label="Ngôn ngữ đích"
          icon={<Globe className="w-4 h-4 text-cyan-400" />}
          label="Dịch sang"
          display={LANG_LABELS[targetLang] ?? targetLang}
          value={targetLang}
          onChange={(e) => onSelectTargetLang(e.target.value)}
          disabled={isRunning}
        >
          <option value="vi">Tiếng Việt (VI)</option>
          <option value="en">English (EN)</option>
          <option value="ja">日本語 (JA)</option>
          <option value="ko">한국어 (KO)</option>
          <option value="zh">中文 (ZH)</option>
        </SelectChip>

        <SelectChip
          aria-label="Kiểu phụ đề"
          icon={<Palette className="w-4 h-4 text-amber-400" />}
          label="Kiểu phụ đề"
          display={PRESET_LABELS[stylePreset] ?? stylePreset}
          value={stylePreset}
          onChange={(e) => onSelectPreset(e.target.value as StylePreset)}
          disabled={isRunning}
        >
          <option value="youtube_yellow">YouTube Yellow</option>
          <option value="classic_white">Classic White</option>
          <option value="netflix_modern">Netflix Modern</option>
          <option value="bilingual_highlight">Bilingual Highlight</option>
        </SelectChip>

        {/* Bilingual switch, same height as the chips */}
        <button
          aria-pressed={bilingual}
          aria-label="Song ngữ"
          onClick={onToggleBilingual}
          disabled={isRunning}
          className="h-10 flex items-center gap-2.5 pl-3 pr-3.5 rounded-lg border border-slate-700/80 bg-slate-900 hover:border-slate-500 hover:bg-slate-800/60 transition-colors disabled:opacity-50"
          title="Bật/Tắt chế độ phụ đề song ngữ (cả tiếng gốc và bản dịch)"
        >
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
          <span className="text-xs font-medium text-slate-200">Song ngữ</span>
        </button>
      </div>
    </header>
  );
};
