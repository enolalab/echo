import React, { useState } from 'react';
import { Terminal, ChevronUp, ChevronDown, CheckCircle2, Loader2 } from 'lucide-react';
import type { PipelineStageEvent } from '../types';

interface ProgressDrawerProps {
  currentStage: PipelineStageEvent | null;
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

export const ProgressDrawer: React.FC<ProgressDrawerProps> = ({
  currentStage,
  logs,
  isRunning,
  onOpenExport,
  canExport,
}) => {
  const [showLogs, setShowLogs] = useState(false);

  const getStageIndex = (stageKey?: string) => {
    if (!stageKey) return -1;
    return STAGES.findIndex((s) => s.key === stageKey);
  };

  const currentIndex = getStageIndex(currentStage?.stage);

  return (
    <div className="bg-slate-900 border-t border-slate-800 text-xs z-10 transition-all select-none">
      {/* Status Bar */}
      <div className="h-12 px-4 flex items-center justify-between gap-4">
        {/* Stages breadcrumb */}
        <div className="flex items-center gap-2">
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

        {/* Current status text */}
        <div className="flex-1 text-center truncate text-slate-400 font-mono text-[11px]">
          {currentStage?.message || (isRunning ? 'Đang khởi chạy...' : 'Sẵn sàng')}
        </div>

        {/* Action Buttons: Log Toggle & Export */}
        <div className="flex items-center gap-2">
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
        <div className="h-36 bg-black/90 border-t border-slate-800 p-3 font-mono text-[11px] text-slate-300 overflow-y-auto flex flex-col gap-1">
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
      )}
    </div>
  );
};
