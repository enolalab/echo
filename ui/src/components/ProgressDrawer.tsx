import React, { useState } from 'react';
import { Terminal, ChevronUp, ChevronDown, CheckCircle2, Loader2, Copy, Check } from 'lucide-react';
import type { PipelineStageEvent, PipelineProgressEvent } from '../types';

interface ProgressDrawerProps {
  currentStage: PipelineStageEvent | null;
  progress: PipelineProgressEvent | null;
  logs: string[];
  isRunning: boolean;
  onOpenExport: () => void;
  canExport: boolean;
}

const STAGES = [
  { key: 'extract_audio', label: 'Tách Audio' },
  { key: 'stt', label: 'Whisper STT' },
  { key: 'translate', label: 'LLM Dịch' },
  { key: 'ready', label: 'Sẵn Sàng' },
] as const;

function formatElapsed(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
}

export const ProgressDrawer: React.FC<ProgressDrawerProps> = ({
  currentStage,
  progress,
  logs,
  isRunning,
  onOpenExport,
  canExport,
}) => {
  const [showLogs, setShowLogs] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleCopyLogs = () => {
    const text = logs.join('\n');
    if (navigator.clipboard?.writeText) {
      navigator.clipboard.writeText(text).then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      });
    }
  };

  const activeStageKey = progress?.stage || currentStage?.stage;
  const getStageIndex = (stageKey?: string) => {
    if (!stageKey) return -1;
    return STAGES.findIndex((s) => s.key === stageKey);
  };

  const currentIndex = getStageIndex(activeStageKey);
  const percentage = progress?.percentage ?? (currentIndex === 3 ? 100 : isRunning ? 5 : 0);

  return (
    <div className="bg-slate-900 border-t border-slate-800 text-xs z-10 transition-all select-none">
      {/* 3px Top Gradient Progress Bar */}
      <div className="w-full bg-slate-800/60 h-[3px] overflow-hidden relative">
        <div
          data-testid="progress-line"
          className={`h-full transition-all duration-300 ease-out ${
            percentage >= 100
              ? 'bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.5)]'
              : isRunning
              ? 'bg-gradient-to-r from-blue-500 via-indigo-500 to-emerald-400 shadow-[0_0_8px_rgba(59,130,246,0.5)]'
              : 'bg-slate-700'
          }`}
          style={{ width: `${Math.min(100, Math.max(0, percentage))}%` }}
        />
      </div>

      {/* Status Bar */}
      <div className="h-12 px-4 flex items-center justify-between gap-4">
        {/* Stages breadcrumb */}
        <div className="flex items-center gap-2 shrink-0">
          {STAGES.map((s, idx) => {
            const isDone = currentIndex > idx || (currentIndex === 3 && idx === 3);
            const isCurrent = currentIndex === idx && isRunning;

            return (
              <React.Fragment key={s.key}>
                <div
                  className={`flex items-center gap-1.5 px-2.5 py-1 rounded-full text-[11px] font-medium border ${
                    isDone
                      ? 'bg-emerald-950/40 text-emerald-400 border-emerald-800/50'
                      : isCurrent
                      ? 'bg-blue-950/60 text-blue-300 border-blue-500/80 animate-pulse'
                      : 'bg-slate-800/40 text-slate-500 border-slate-700/40'
                  }`}
                >
                  {isDone ? (
                    <CheckCircle2 className="w-3 h-3 text-emerald-400" />
                  ) : isCurrent ? (
                    <Loader2 className="w-3 h-3 animate-spin text-blue-400" />
                  ) : (
                    <span className="w-1.5 h-1.5 rounded-full bg-slate-600" />
                  )}
                  <span>{s.label}</span>
                </div>

                {idx < STAGES.length - 1 && <span className="text-slate-700">➔</span>}
              </React.Fragment>
            );
          })}
        </div>

        {/* Current status & Progress badge */}
        <div className="flex-1 flex items-center justify-center gap-2 truncate px-2 font-mono text-[11px]">
          {isRunning && (
            <span
              data-testid="progress-percentage-badge"
              className="bg-blue-950/80 text-blue-400 font-bold px-1.5 py-0.5 rounded border border-blue-800/50 shrink-0"
            >
              {Math.round(percentage)}%
            </span>
          )}

          <span className="truncate text-slate-300">
            {progress?.message || currentStage?.message || (isRunning ? 'Đang khởi chạy...' : 'Sẵn sàng')}
          </span>

          {isRunning && progress && progress.elapsed_secs > 0 && (
            <span
              data-testid="progress-timer"
              className="text-slate-400 bg-slate-800/70 px-1.5 py-0.5 rounded border border-slate-700/60 text-[10px] shrink-0"
            >
              ⏱ {formatElapsed(progress.elapsed_secs)}
            </span>
          )}
        </div>

        {/* Action Buttons: Log Toggle & Export */}
        <div className="flex items-center gap-2 shrink-0">
          <button
            onClick={() => setShowLogs(!showLogs)}
            className="flex items-center gap-1 text-slate-400 hover:text-slate-200 bg-slate-800 px-2.5 py-1 rounded border border-slate-700 transition"
          >
            <Terminal className="w-3.5 h-3.5" />
            <span>Logs ({logs.length})</span>
            {showLogs ? <ChevronDown className="w-3 h-3" /> : <ChevronUp className="w-3 h-3" />}
          </button>

          <button
            onClick={onOpenExport}
            disabled={!canExport || isRunning}
            className={`font-semibold px-4 py-1.5 rounded-md shadow transition flex items-center gap-1.5 ${
              canExport && !isRunning
                ? 'bg-emerald-600 hover:bg-emerald-500 text-white cursor-pointer'
                : 'bg-slate-800 text-slate-600 border border-slate-700 cursor-not-allowed'
            }`}
          >
            <span>🎬 Xuất Video</span>
          </button>
        </div>
      </div>

      {/* Expandable Console Logs */}
      {showLogs && (
        <div className="relative h-36 bg-black/90 border-t border-slate-800">
          <button
            onClick={handleCopyLogs}
            className="absolute top-2 right-2 z-10 flex items-center gap-1 bg-slate-800/90 hover:bg-slate-700 text-slate-300 px-2 py-1 rounded border border-slate-600 text-[10px] transition"
            title="Copy toàn bộ log"
          >
            {copied ? (
              <>
                <Check className="w-3 h-3 text-emerald-400" />
                <span className="text-emerald-400">Đã copy</span>
              </>
            ) : (
              <>
                <Copy className="w-3 h-3" />
                <span>Copy</span>
              </>
            )}
          </button>
          <div className="h-full p-3 font-mono text-[11px] text-slate-300 overflow-y-auto flex flex-col gap-1 select-text cursor-text">
          {logs.length > 0 ? (
            logs.map((log, i) => (
              <div key={i} className="leading-relaxed">
                <span className="text-slate-600 select-none mr-2">{i + 1}</span>
                <span>{log}</span>
              </div>
            ))
          ) : (
            <div className="text-slate-600 italic">Chưa có thông điệp log nào.</div>
          )}
          </div>
        </div>
      )}
    </div>
  );
};
