import React, { useState } from 'react';
import { Trash2, Scissors, ArrowDownToLine, Plus, Minus } from 'lucide-react';
import type { SubtitleSegment } from '../types';

interface SubtitleRowProps {
  segment: SubtitleSegment;
  isActive: boolean;
  onSeek: (ms: number) => void;
  onUpdate: (updated: SubtitleSegment) => void;
  onDelete: (id: number) => void;
  onSplit: (id: number) => void;
  onMergeWithNext: (id: number) => void;
  isLast: boolean;
}

function formatTimestamp(ms: number): string {
  const totalSeconds = ms / 1000;
  const m = Math.floor(totalSeconds / 60);
  const s = Math.floor(totalSeconds % 60);
  const milli = ms % 1000;
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}.${Math.floor(milli / 100)}`;
}

export const SubtitleRow: React.FC<SubtitleRowProps> = ({
  segment,
  isActive,
  onSeek,
  onUpdate,
  onDelete,
  onSplit,
  onMergeWithNext,
  isLast,
}) => {
  const [origText, setOrigText] = useState(segment.original_text);
  const [transText, setTransText] = useState(segment.translated_text || '');

  const handleOrigBlur = () => {
    if (origText !== segment.original_text) {
      onUpdate({ ...segment, original_text: origText });
    }
  };

  const handleTransBlur = () => {
    if (transText !== (segment.translated_text || '')) {
      onUpdate({ ...segment, translated_text: transText || null });
    }
  };

  const nudgeStart = (deltaMs: number, e: React.MouseEvent) => {
    e.stopPropagation();
    const newStart = Math.max(0, segment.start_ms + deltaMs);
    if (newStart < segment.end_ms) {
      onUpdate({ ...segment, start_ms: newStart });
    }
  };

  const nudgeEnd = (deltaMs: number, e: React.MouseEvent) => {
    e.stopPropagation();
    const newEnd = segment.end_ms + deltaMs;
    if (newEnd > segment.start_ms) {
      onUpdate({ ...segment, end_ms: newEnd });
    }
  };

  return (
    <div
      id={`sub-row-${segment.id}`}
      onClick={() => onSeek(segment.start_ms)}
      className={`group p-2.5 rounded-lg border text-xs transition-colors cursor-pointer flex flex-col gap-1.5 ${
        isActive
          ? 'bg-blue-950/40 border-blue-500/70 shadow-sm shadow-blue-500/10'
          : 'bg-slate-800/60 hover:bg-slate-800 border-slate-700/60'
      }`}
    >
      {/* Header: ID, Timing & Actions */}
      <div className="flex items-center justify-between text-slate-400 text-[11px]">
        <div className="flex items-center gap-2">
          <span className="font-mono font-semibold text-slate-500">#{segment.id}</span>

          {/* Start time with nudge */}
          <div className="flex items-center gap-0.5 bg-slate-900/60 px-1.5 py-0.5 rounded border border-slate-700">
            <span className="font-mono text-slate-300">{formatTimestamp(segment.start_ms)}</span>
            <button
              onClick={(e) => nudgeStart(-100, e)}
              className="hover:text-blue-400 p-0.5"
              title="Lùi 0.1s"
            >
              <Minus className="w-2.5 h-2.5" />
            </button>
            <button
              onClick={(e) => nudgeStart(100, e)}
              className="hover:text-blue-400 p-0.5"
              title="Tiến 0.1s"
            >
              <Plus className="w-2.5 h-2.5" />
            </button>
          </div>

          <span className="text-slate-600">→</span>

          {/* End time with nudge */}
          <div className="flex items-center gap-0.5 bg-slate-900/60 px-1.5 py-0.5 rounded border border-slate-700">
            <span className="font-mono text-slate-300">{formatTimestamp(segment.end_ms)}</span>
            <button
              onClick={(e) => nudgeEnd(-100, e)}
              className="hover:text-blue-400 p-0.5"
              title="Lùi 0.1s"
            >
              <Minus className="w-2.5 h-2.5" />
            </button>
            <button
              onClick={(e) => nudgeEnd(100, e)}
              className="hover:text-blue-400 p-0.5"
              title="Tiến 0.1s"
            >
              <Plus className="w-2.5 h-2.5" />
            </button>
          </div>
        </div>

        {/* Row Operations */}
        <div className="flex items-center gap-1 opacity-60 group-hover:opacity-100 transition-opacity">
          <button
            onClick={(e) => {
              e.stopPropagation();
              onSplit(segment.id);
            }}
            className="p-1 hover:text-amber-400 transition"
            title="Tách đôi câu này (Split)"
          >
            <Scissors className="w-3 h-3" />
          </button>

          {!isLast && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onMergeWithNext(segment.id);
              }}
              className="p-1 hover:text-indigo-400 transition"
              title="Gộp với câu tiếp theo (Merge)"
            >
              <ArrowDownToLine className="w-3 h-3" />
            </button>
          )}

          <button
            onClick={(e) => {
              e.stopPropagation();
              onDelete(segment.id);
            }}
            className="p-1 hover:text-rose-400 transition"
            title="Xóa câu này"
          >
            <Trash2 className="w-3 h-3" />
          </button>
        </div>
      </div>

      {/* Editable Text Area: Original */}
      <input
        type="text"
        value={origText}
        onChange={(e) => setOrigText(e.target.value)}
        onBlur={handleOrigBlur}
        onClick={(e) => e.stopPropagation()}
        className="w-full bg-slate-900/50 focus:bg-slate-900 border border-slate-700/60 focus:border-blue-500 rounded px-2 py-1 text-slate-300 focus:outline-none transition-colors"
        placeholder="Câu gốc..."
      />

      {/* Editable Text Area: Translated */}
      <input
        type="text"
        value={transText}
        onChange={(e) => setTransText(e.target.value)}
        onBlur={handleTransBlur}
        onClick={(e) => e.stopPropagation()}
        className="w-full bg-slate-900/80 focus:bg-slate-900 border border-slate-700/80 focus:border-amber-400 rounded px-2 py-1 text-amber-300 font-medium focus:outline-none transition-colors"
        placeholder="Bản dịch tiếng Việt..."
      />
    </div>
  );
};
