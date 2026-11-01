import React from 'react';
import type { SubtitleSegment, StylePreset } from '../types';
import { PRESET_STYLES } from '../lib/presets';

interface SubtitleOverlayProps {
  currentTimeMs: number;
  segments: SubtitleSegment[];
  preset: StylePreset;
  isBilingual: boolean;
}

export const SubtitleOverlay: React.FC<SubtitleOverlayProps> = ({
  currentTimeMs,
  segments,
  preset,
  isBilingual,
}) => {
  const activeSegment = segments.find(
    (seg) => currentTimeMs >= seg.start_ms && currentTimeMs <= seg.end_ms
  );

  if (!activeSegment) {
    return null;
  }

  const styleDef = PRESET_STYLES[preset] || PRESET_STYLES.youtube_yellow;
  const primaryText = activeSegment.translated_text || activeSegment.original_text;
  const hasSecondary = isBilingual && activeSegment.translated_text && activeSegment.original_text;

  return (
    <div
      data-testid="subtitle-overlay"
      className={`absolute left-0 right-0 pointer-events-none flex flex-col items-center justify-center transition-opacity duration-150 ${styleDef.containerClass}`}
    >
      <div className={`text-xl md:text-2xl max-w-[90%] leading-tight text-center ${styleDef.primaryClass}`}>
        {primaryText}
      </div>

      {hasSecondary && (
        <div className={`max-w-[85%] leading-snug text-center ${styleDef.secondaryClass}`}>
          {activeSegment.original_text}
        </div>
      )}
    </div>
  );
};
