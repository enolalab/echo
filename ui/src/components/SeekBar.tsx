import React from 'react';
import type { SubtitleSegment } from '../types';

interface SeekBarProps {
  durationMs: number;
  segments: SubtitleSegment[];
  currentTimeMs: number;
  onSeek: (ms: number) => void;
}

export const SeekBar: React.FC<SeekBarProps> = ({ durationMs, segments, currentTimeMs, onSeek }) => {
  const pct = (ms: number) => Math.min(Math.max((ms / durationMs) * 100, 0), 100);

  const handleClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (durationMs <= 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    if (rect.width <= 0) return;
    const ms = Math.round(((e.clientX - rect.left) / rect.width) * durationMs);
    onSeek(Math.min(Math.max(ms, 0), durationMs));
  };

  return (
    <div
      role="slider"
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={durationMs}
      aria-valuenow={currentTimeMs}
      onClick={handleClick}
      className="relative w-full h-2.5 bg-slate-700 rounded cursor-pointer overflow-hidden"
    >
      {durationMs > 0 &&
        segments.map((s) => (
          <div
            key={s.id}
            data-testid={`seg-${s.id}`}
            className={`absolute top-0 h-full ${
              currentTimeMs >= s.start_ms && currentTimeMs <= s.end_ms
                ? 'bg-blue-400/80'
                : 'bg-slate-500/60'
            }`}
            style={{
              left: `${pct(s.start_ms)}%`,
              width: `${Math.max(pct(s.end_ms) - pct(s.start_ms), 0)}%`,
              minWidth: '2px',
            }}
          />
        ))}
      {durationMs > 0 && (
        <div
          data-testid="playhead"
          className="absolute top-0 w-0.5 h-full bg-blue-300"
          style={{ left: `${pct(currentTimeMs)}%` }}
        />
      )}
    </div>
  );
};
