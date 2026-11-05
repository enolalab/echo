import React, { useState, useEffect } from 'react';
import {
  X,
  Play,
  Mic,
  Languages,
  GraduationCap,
  Globe,
  Palette,
  Cpu,
  Zap,
  Download,
  Loader2,
  FileVideo,
  Film,
  Sliders,
  AlertTriangle,
} from 'lucide-react';
import type {
  GeneralSettings,
  ProfileView,
  Skill,
  StylePreset,
  SystemInfo,
  BatchJob,
} from '../types';
import { KIND_ICONS } from '../lib/providers';

export interface RunConfigPayload {
  generalPatch: Partial<GeneralSettings>;
  profileId: string | null;
  skillId: string | null;
}

interface RunConfigModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (payload: RunConfigPayload) => void;
  queuedJobs: BatchJob[];
  currentVideoPath?: string;
  general: GeneralSettings;
  systemInfo: SystemInfo | null;
  profiles: ProfileView[];
  defaultProfileId: string | null;
  presets: Skill[];
  userSkills: Skill[];
  defaultSkillId: string | null;
  onDownloadModel: (name: string) => void;
  downloadingModel: string | null;
}

const LANG_OPTIONS = [
  { value: 'vi', label: 'Tiếng Việt (VI)' },
  { value: 'en', label: 'English (EN)' },
  { value: 'ja', label: '日本語 (JA)' },
  { value: 'ko', label: '한국어 (KO)' },
  { value: 'zh', label: '中文 (ZH)' },
];

const PRESET_OPTIONS: { value: StylePreset; label: string }[] = [
  { value: 'youtube_yellow', label: 'YouTube Yellow' },
  { value: 'classic_white', label: 'Classic White' },
  { value: 'netflix_modern', label: 'Netflix Modern' },
  { value: 'bilingual_highlight', label: 'Bilingual Highlight' },
];

function getFileBaseName(path: string): string {
  const clean = path.replace(/^blob:/, '');
  return clean.split(/[/\\]/).pop() || clean;
}

export const RunConfigModal: React.FC<RunConfigModalProps> = ({
  isOpen,
  onClose,
  onConfirm,
  queuedJobs,
  currentVideoPath,
  general,
  systemInfo,
  profiles,
  defaultProfileId,
  presets,
  userSkills,
  defaultSkillId,
  onDownloadModel,
  downloadingModel,
}) => {
  // Local draft states
  const [whisperModel, setWhisperModel] = useState(general.whisper_model);
  const [targetLang, setTargetLang] = useState(general.target_lang);
  const [bilingual, setBilingual] = useState(general.bilingual);
  const [stylePreset, setStylePreset] = useState<StylePreset>(general.style_preset);
  const [preferGpu, setPreferGpu] = useState(general.prefer_gpu);
  const [autoExport, setAutoExport] = useState(general.auto_export);
  const [padSubs, setPadSubs] = useState(general.pad_subs);
  const [dubEnabled, setDubEnabled] = useState(general.dub_enabled);
  const [profileId, setProfileId] = useState<string | null>(defaultProfileId);
  const [skillId, setSkillId] = useState<string | null>(defaultSkillId);

  // Sync draft with current values when modal opens
  useEffect(() => {
    if (isOpen) {
      setWhisperModel(general.whisper_model);
      setTargetLang(general.target_lang);
      setBilingual(general.bilingual);
      setStylePreset(general.style_preset);
      setPreferGpu(general.prefer_gpu);
      setAutoExport(general.auto_export);
      setPadSubs(general.pad_subs);
      setDubEnabled(general.dub_enabled);
      setProfileId(defaultProfileId);
      setSkillId(defaultSkillId);
    }
  }, [isOpen, general, defaultProfileId, defaultSkillId]);

  // Handle ESC key to close
  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const currentWhisperInfo = systemInfo?.whisper_models.find((m) => m.name === whisperModel);
  const isDownloaded = currentWhisperInfo?.downloaded ?? true;

  // Determine list of files to run
  const filesToRun =
    queuedJobs.length > 0
      ? queuedJobs.map((j) => getFileBaseName(j.videoPath))
      : currentVideoPath
        ? [getFileBaseName(currentVideoPath)]
        : [];

  const handleStartRun = () => {
    onConfirm({
      generalPatch: {
        whisper_model: whisperModel,
        target_lang: targetLang,
        bilingual,
        style_preset: stylePreset,
        prefer_gpu: preferGpu,
        auto_export: autoExport,
        pad_subs: padSubs,
        dub_enabled: dubEnabled,
      },
      profileId,
      skillId,
    });
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="run-config-title"
      className="fixed inset-0 z-50 bg-black/75 backdrop-blur-sm flex items-center justify-center p-4 animate-in fade-in duration-150"
      onClick={onClose}
    >
      <div
        className="bg-slate-900 border border-slate-700/80 rounded-xl shadow-2xl max-w-xl w-full flex flex-col overflow-hidden max-h-[90vh]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="px-5 py-4 border-b border-slate-800 flex items-center justify-between bg-slate-950/60 shrink-0">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-blue-600/20 border border-blue-500/30 flex items-center justify-center text-blue-400">
              <Sliders className="w-4 h-4" />
            </div>
            <div>
              <h2 id="run-config-title" className="text-sm font-semibold text-white">
                Xác nhận cấu hình trước khi chạy
              </h2>
              <p className="text-xs text-slate-400 mt-0.5">
                Kiểm tra thông số AI và tùy chọn xử lý video
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            aria-label="Đóng"
            className="text-slate-400 hover:text-white p-1 rounded-md hover:bg-slate-800 transition"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Scrollable Content */}
        <div className="p-5 space-y-4 overflow-y-auto min-h-0 text-xs">
          {/* Files Summary */}
          <div className="bg-slate-950/50 border border-slate-800/80 rounded-lg p-3">
            <div className="flex items-center justify-between mb-2">
              <span className="text-[11px] font-semibold text-slate-400 uppercase tracking-wide flex items-center gap-1.5">
                <FileVideo className="w-3.5 h-3.5 text-blue-400" />
                Danh sách tệp xử lý ({filesToRun.length})
              </span>
              <span className="text-[11px] text-blue-400 font-medium">Sắp chạy</span>
            </div>
            {filesToRun.length === 0 ? (
              <p className="text-slate-500 italic text-[11px]">Chưa có video nào được chọn.</p>
            ) : (
              <div className="max-h-24 overflow-y-auto space-y-1.5 pr-1">
                {filesToRun.map((name, idx) => (
                  <div
                    key={idx}
                    className="flex items-center gap-2 text-slate-300 font-mono text-[11px] bg-slate-900/80 px-2 py-1 rounded border border-slate-800"
                  >
                    <span className="text-slate-500 text-[10px] w-4">{idx + 1}.</span>
                    <span className="truncate flex-1" title={name}>
                      {name}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Section 1: Nhận dạng giọng nói (STT) */}
          <div className="space-y-3">
            <div className="text-[11px] font-semibold text-slate-400 uppercase tracking-wider flex items-center gap-1.5">
              <Mic className="w-3.5 h-3.5 text-sky-400" />
              1. Nhận dạng giọng nói (Whisper STT)
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              {/* Whisper Model Select */}
              <div className="space-y-1">
                <label htmlFor="modal-whisper-model" className="text-slate-300 font-medium block">
                  Mô hình Whisper
                </label>
                <div className="flex gap-1.5">
                  <select
                    id="modal-whisper-model"
                    aria-label="Mô hình Whisper"
                    value={whisperModel}
                    onChange={(e) => setWhisperModel(e.target.value)}
                    className="flex-1 bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1.5 text-slate-200 focus:outline-none focus:border-blue-500"
                  >
                    {systemInfo?.whisper_models.map((m) => (
                      <option key={m.name} value={m.name}>
                        {m.name} ({m.size_mb}MB){m.downloaded ? ' ✓' : ''}
                      </option>
                    )) || <option value="base">base</option>}
                  </select>

                  {!isDownloaded && (
                    <button
                      type="button"
                      onClick={() => onDownloadModel(whisperModel)}
                      disabled={downloadingModel === whisperModel}
                      className="px-2.5 py-1.5 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-1 text-[11px] font-medium transition shrink-0"
                      title="Tải model về máy"
                    >
                      {downloadingModel === whisperModel ? (
                        <Loader2 className="w-3.5 h-3.5 animate-spin" />
                      ) : (
                        <Download className="w-3.5 h-3.5" />
                      )}
                      <span>Tải</span>
                    </button>
                  )}
                </div>
              </div>

              {/* Hardware Acceleration (GPU/CPU) */}
              <div className="space-y-1">
                <label className="text-slate-300 font-medium block">Phần cứng xử lý</label>
                {systemInfo?.has_cuda ? (
                  <button
                    type="button"
                    onClick={() => setPreferGpu(!preferGpu)}
                    className={`w-full h-[34px] px-3 rounded-lg border flex items-center justify-between text-left transition ${
                      preferGpu
                        ? systemInfo?.cuda_ready === false
                          ? 'bg-amber-950/40 border-amber-700/60 text-amber-300'
                          : 'bg-emerald-950/40 border-emerald-700/60 text-emerald-300'
                        : 'bg-slate-950 border-slate-700 text-slate-300'
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      {preferGpu ? (
                        systemInfo?.cuda_ready === false ? (
                          <AlertTriangle className="w-3.5 h-3.5 text-amber-400 shrink-0" />
                        ) : (
                          <Zap className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                        )
                      ) : (
                        <Cpu className="w-3.5 h-3.5 text-slate-400 shrink-0" />
                      )}
                      <span className="font-medium text-xs">
                        {preferGpu
                          ? systemInfo?.cuda_ready === false
                            ? 'CUDA (Cảnh báo driver)'
                            : 'NVIDIA GPU (CUDA)'
                          : 'CPU (Đa luồng)'}
                      </span>
                    </div>
                    <span className="text-[10px] text-slate-500 underline">Đổi</span>
                  </button>
                ) : (
                  <div className="h-[34px] px-3 rounded-lg border border-slate-800 bg-slate-950/50 flex items-center gap-2 text-slate-400">
                    <Cpu className="w-3.5 h-3.5" />
                    <span>Chỉ CPU (Không có CUDA)</span>
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Section 2: Dịch thuật AI (LLM) */}
          <div className="space-y-3 pt-1">
            <div className="text-[11px] font-semibold text-slate-400 uppercase tracking-wider flex items-center gap-1.5">
              <Languages className="w-3.5 h-3.5 text-indigo-400" />
              2. Dịch thuật AI & Ngôn ngữ
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
              {/* Profile Select */}
              <div className="space-y-1">
                <label htmlFor="modal-profile" className="text-slate-300 font-medium block">
                  AI Dịch thuật
                </label>
                <select
                  id="modal-profile"
                  aria-label="AI Dịch thuật"
                  value={profileId ?? ''}
                  onChange={(e) => setProfileId(e.target.value || null)}
                  className="w-full bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1.5 text-slate-200 focus:outline-none focus:border-blue-500"
                >
                  {profiles.length === 0 && <option value="">Chưa có profile</option>}
                  {profiles.map((p) => (
                    <option key={p.id} value={p.id}>
                      {KIND_ICONS[p.kind]} {p.name} · {p.model}
                    </option>
                  ))}
                </select>
              </div>

              {/* Target Language */}
              <div className="space-y-1">
                <label htmlFor="modal-target-lang" className="text-slate-300 font-medium block flex items-center gap-1">
                  <Globe className="w-3 h-3 text-cyan-400" />
                  Dịch sang
                </label>
                <select
                  id="modal-target-lang"
                  aria-label="Ngôn ngữ dịch sang"
                  value={targetLang}
                  onChange={(e) => setTargetLang(e.target.value)}
                  className="w-full bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1.5 text-slate-200 focus:outline-none focus:border-blue-500"
                >
                  {LANG_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>

              {/* Skill */}
              <div className="space-y-1">
                <label htmlFor="modal-skill" className="text-slate-300 font-medium block flex items-center gap-1">
                  <GraduationCap className="w-3 h-3 text-emerald-400" />
                  Kỹ năng (Skill)
                </label>
                <select
                  id="modal-skill"
                  aria-label="Kỹ năng chuyên môn"
                  value={skillId ?? 'none'}
                  onChange={(e) => setSkillId(e.target.value === 'none' ? null : e.target.value)}
                  className="w-full bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1.5 text-slate-200 focus:outline-none focus:border-blue-500"
                >
                  <option value="none">Không dùng</option>
                  {presets.length > 0 && (
                    <optgroup label="Có sẵn">
                      {presets.map((s) => (
                        <option key={s.id} value={s.id}>
                          🔒 {s.name}
                        </option>
                      ))}
                    </optgroup>
                  )}
                  {userSkills.length > 0 && (
                    <optgroup label="Của tôi">
                      {userSkills.map((s) => (
                        <option key={s.id} value={s.id}>
                          {s.name}
                        </option>
                      ))}
                    </optgroup>
                  )}
                </select>
              </div>
            </div>
          </div>

          {/* Section 3: Tùy chọn Xuất & Phụ đề */}
          <div className="space-y-3 pt-1">
            <div className="text-[11px] font-semibold text-slate-400 uppercase tracking-wider flex items-center gap-1.5">
              <Film className="w-3.5 h-3.5 text-amber-400" />
              3. Định dạng & Xuất video
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              {/* Style Preset */}
              <div className="space-y-1">
                <label htmlFor="modal-style-preset" className="text-slate-300 font-medium block flex items-center gap-1">
                  <Palette className="w-3 h-3 text-amber-400" />
                  Kiểu phụ đề
                </label>
                <select
                  id="modal-style-preset"
                  aria-label="Kiểu phụ đề"
                  value={stylePreset}
                  onChange={(e) => setStylePreset(e.target.value as StylePreset)}
                  className="w-full bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1.5 text-slate-200 focus:outline-none focus:border-blue-500"
                >
                  {PRESET_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>

              {/* Bilingual Option */}
              <div className="flex items-center justify-between p-2.5 rounded-lg border border-slate-800 bg-slate-950/60 mt-auto h-[34px]">
                <span className="text-slate-200 font-medium">Phụ đề song ngữ</span>
                <button
                  type="button"
                  role="switch"
                  aria-checked={bilingual}
                  onClick={() => setBilingual(!bilingual)}
                  className={`relative w-8 h-[18px] rounded-full transition-colors shrink-0 ${
                    bilingual ? 'bg-blue-600' : 'bg-slate-700'
                  }`}
                >
                  <span
                    className={`absolute top-[2px] left-[2px] w-[14px] h-[14px] rounded-full bg-white shadow transition-transform ${
                      bilingual ? 'translate-x-[14px]' : ''
                    }`}
                  />
                </button>
              </div>
            </div>

            {/* Auto export hardsub & Pad subs */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-1">
              <div className="flex items-start gap-2.5 p-2.5 rounded-lg border border-slate-800 bg-slate-950/40">
                <input
                  id="modal-auto-export"
                  type="checkbox"
                  checked={autoExport}
                  onChange={(e) => setAutoExport(e.target.checked)}
                  className="mt-0.5 rounded border-slate-700 bg-slate-900 text-blue-600 focus:ring-blue-500"
                />
                <label htmlFor="modal-auto-export" className="cursor-pointer select-none">
                  <span className="font-medium text-slate-200 block">Tự động xuất video (Hardsub)</span>
                  <span className="text-[11px] text-slate-400 block mt-0.5 leading-tight">
                    Render luôn video ra thư mục output sau khi dịch xong
                  </span>
                </label>
              </div>

              <div
                className={`flex items-start gap-2.5 p-2.5 rounded-lg border transition ${
                  autoExport
                    ? 'border-slate-800 bg-slate-950/40'
                    : 'border-slate-800/40 bg-slate-950/20 opacity-50 cursor-not-allowed'
                }`}
              >
                <input
                  id="modal-pad-subs"
                  type="checkbox"
                  disabled={!autoExport}
                  checked={padSubs}
                  onChange={(e) => setPadSubs(e.target.checked)}
                  className="mt-0.5 rounded border-slate-700 bg-slate-900 text-blue-600 focus:ring-blue-500 disabled:cursor-not-allowed"
                />
                <label
                  htmlFor="modal-pad-subs"
                  className={`select-none ${autoExport ? 'cursor-pointer' : 'cursor-not-allowed'}`}
                >
                  <span className="font-medium text-slate-200 block">Đệm đen dưới đáy video</span>
                  <span className="text-[11px] text-slate-400 block mt-0.5 leading-tight">
                    Thêm dải đen bên dưới tránh chữ đè lên nội dung video
                  </span>
                </label>
              </div>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="px-5 py-3.5 border-t border-slate-800 bg-slate-950/70 flex items-center justify-between shrink-0">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition font-medium text-xs"
          >
            Hủy bỏ
          </button>

          <button
            type="button"
            onClick={handleStartRun}
            className="flex items-center gap-2 px-5 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white font-semibold text-xs shadow-lg shadow-blue-600/30 transition cursor-pointer"
          >
            <Play className="w-3.5 h-3.5 fill-current" />
            <span>Bắt đầu chạy ({filesToRun.length})</span>
          </button>
        </div>
      </div>
    </div>
  );
};
