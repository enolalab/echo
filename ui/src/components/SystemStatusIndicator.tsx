import React, { useState, useRef, useEffect } from 'react';
import {
  CheckCircle2,
  AlertTriangle,
  AlertCircle,
  Settings as SettingsIcon,
  Download,
  Loader2,
  Cpu,
  Zap,
} from 'lucide-react';
import type { SystemInfo, ProfileView } from '../types';

export interface SystemStatusIndicatorProps {
  systemInfo: SystemInfo | null;
  whisperModel: string;
  profiles: ProfileView[];
  defaultProfileId: string | null;
  preferGpu: boolean;
  onOpenSettings: () => void;
  onDownloadModel?: (model: string) => void;
  downloadingModel?: boolean | string | null;
}

export const SystemStatusIndicator: React.FC<SystemStatusIndicatorProps> = ({
  systemInfo,
  whisperModel,
  profiles,
  defaultProfileId,
  preferGpu,
  onOpenSettings,
  onDownloadModel,
  downloadingModel,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const isDownloading = Boolean(downloadingModel);

  // Close popover when clicking outside
  useEffect(() => {
    if (!isOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isOpen]);

  const ffmpegOk = systemInfo?.ffmpeg_ready ?? true;
  const currentWhisperInfo = systemInfo?.whisper_models.find((m) => m.name === whisperModel);
  const modelDownloaded = currentWhisperInfo?.downloaded ?? true;
  const currentProfile = profiles.find((p) => p.id === defaultProfileId);
  const profileOk = Boolean(currentProfile || profiles.length > 0);

  const gpuBroken = preferGpu && Boolean(systemInfo?.has_cuda) && systemInfo?.cuda_ready === false;
  const gpuErrorMsg = systemInfo?.cuda_error || 'Driver NVIDIA không phản hồi — chuyển sang CPU';

  let overallStatus: 'ready' | 'warning' | 'error' = 'ready';
  let badgeLabel = 'Sẵn sàng';

  if (!ffmpegOk) {
    overallStatus = 'error';
    badgeLabel = 'Thiếu FFmpeg';
  } else if (!modelDownloaded) {
    overallStatus = 'warning';
    badgeLabel = 'Chưa tải model';
  } else if (!profileOk) {
    overallStatus = 'warning';
    badgeLabel = 'Chưa cấu hình dịch';
  } else if (gpuBroken) {
    overallStatus = 'warning';
    badgeLabel = 'Cảnh báo GPU';
  }

  const badgeStyles = {
    ready: 'bg-emerald-950/40 text-emerald-400 border-emerald-800/60 hover:bg-emerald-900/50',
    warning: 'bg-amber-950/50 text-amber-300 border-amber-700/60 hover:bg-amber-900/50',
    error: 'bg-rose-950/50 text-rose-300 border-rose-700/60 hover:bg-rose-900/50',
  }[overallStatus];

  return (
    <div className="relative shrink-0" ref={containerRef}>
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={`flex items-center gap-1.5 h-7 px-2.5 rounded-full text-[11px] font-medium border transition select-none ${badgeStyles}`}
        title="Bấm để xem chi tiết trạng thái hệ thống"
      >
        {overallStatus === 'ready' && <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />}
        {overallStatus === 'warning' && <AlertTriangle className="w-3.5 h-3.5 text-amber-400 shrink-0" />}
        {overallStatus === 'error' && <AlertCircle className="w-3.5 h-3.5 text-rose-400 shrink-0" />}
        <span>{badgeLabel}</span>
      </button>

      {isOpen && (
        <div className="absolute right-0 top-full mt-2 w-80 bg-slate-900 border border-slate-700 rounded-xl shadow-2xl p-3 z-50 text-[11px] flex flex-col gap-2.5 animate-in fade-in zoom-in-95 duration-100">
          <div className="flex items-center justify-between pb-2 border-b border-slate-800">
            <span className="font-semibold text-slate-200">Trạng thái hệ thống</span>
            <span
              className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${
                overallStatus === 'ready'
                  ? 'bg-emerald-950/60 text-emerald-400 border border-emerald-800/50'
                  : overallStatus === 'warning'
                    ? 'bg-amber-950/60 text-amber-300 border border-amber-800/50'
                    : 'bg-rose-950/60 text-rose-300 border border-rose-800/50'
              }`}
            >
              {overallStatus === 'ready' ? 'Tất cả sẵn sàng' : overallStatus === 'warning' ? 'Cần chú ý' : 'Lỗi thành phần'}
            </span>
          </div>

          {/* Checklist */}
          <div className="space-y-2">
            {/* 1. FFmpeg */}
            <div className="flex items-start gap-2">
              {ffmpegOk ? (
                <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
              ) : (
                <AlertCircle className="w-4 h-4 text-rose-400 shrink-0 mt-0.5" />
              )}
              <div className="flex-1 min-w-0">
                <div className="font-medium text-slate-200">FFmpeg (Bộ xử lý đa phương tiện)</div>
                <div className="text-[10px] text-slate-400">
                  {ffmpegOk ? 'Đã cài đặt và sẵn sàng' : 'Chưa tìm thấy FFmpeg — ứng dụng sẽ tự động tải khi bắt đầu'}
                </div>
              </div>
            </div>

            {/* 2. Whisper Model */}
            <div className="flex items-start gap-2">
              {modelDownloaded ? (
                <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
              ) : (
                <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
              )}
              <div className="flex-1 min-w-0">
                <div className="flex items-center justify-between gap-1">
                  <span className="font-medium text-slate-200">Model Whisper: {whisperModel}</span>
                  {!modelDownloaded && onDownloadModel && (
                    <button
                      onClick={() => onDownloadModel(whisperModel)}
                      disabled={isDownloading}
                      className="px-2 py-0.5 rounded bg-blue-600 hover:bg-blue-500 text-white font-medium text-[10px] flex items-center gap-1 transition"
                    >
                      {isDownloading ? <Loader2 className="w-3 h-3 animate-spin" /> : <Download className="w-3 h-3" />}
                      Tải ngay
                    </button>
                  )}
                </div>
                <div className="text-[10px] text-slate-400">
                  {modelDownloaded ? 'Model đã tải về máy' : 'Chưa tải model này — cần tải trước khi chạy'}
                </div>
              </div>
            </div>

            {/* 3. Translation Profile */}
            <div className="flex items-start gap-2">
              {profileOk ? (
                <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
              ) : (
                <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
              )}
              <div className="flex-1 min-w-0">
                <div className="flex items-center justify-between gap-1">
                  <span className="font-medium text-slate-200">
                    AI Dịch thuật: {currentProfile ? currentProfile.name : profiles[0]?.name ?? 'Chưa có'}
                  </span>
                  {!profileOk && (
                    <button
                      onClick={() => {
                        setIsOpen(false);
                        onOpenSettings();
                      }}
                      className="px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-blue-400 font-medium text-[10px] flex items-center gap-1 border border-slate-700 transition"
                    >
                      <SettingsIcon className="w-3 h-3" />
                      Cài đặt
                    </button>
                  )}
                </div>
                <div className="text-[10px] text-slate-400">
                  {profileOk
                    ? `Dùng ${currentProfile?.kind || 'LLM'} (${currentProfile?.model || 'model mặc định'})`
                    : 'Chưa cấu hình API key hoặc provider dịch'}
                </div>
              </div>
            </div>

            {/* 4. Hardware acceleration (GPU / CPU) */}
            <div className="flex items-start gap-2">
              {preferGpu ? (
                gpuBroken ? (
                  <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
                ) : (
                  <Zap className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
                )
              ) : (
                <Cpu className="w-4 h-4 text-slate-400 shrink-0 mt-0.5" />
              )}
              <div className="flex-1 min-w-0">
                <div className="font-medium text-slate-200">
                  {preferGpu ? (gpuBroken ? 'GPU CUDA (Có cảnh báo)' : 'GPU CUDA (Tăng tốc phần cứng)') : 'Xử lý CPU đa luồng'}
                </div>
                <div className="text-[10px] text-slate-400">
                  {preferGpu
                    ? gpuBroken
                      ? gpuErrorMsg
                      : 'Đang ưu tiên xử lý qua NVIDIA CUDA'
                    : 'Chế độ CPU đang bật — an toàn và ổn định trên mọi máy'}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
