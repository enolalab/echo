import { useEffect, useRef } from 'react';
import type { SubtitleSegment } from '../types';

export function useSubtitleSync(
  segments: SubtitleSegment[],
  currentTimeMs: number,
  containerRef: React.RefObject<HTMLDivElement | null>
) {
  const activeSegmentId = segments.find(
    (seg) => currentTimeMs >= seg.start_ms && currentTimeMs <= seg.end_ms
  )?.id;

  const lastScrolledId = useRef<number | null>(null);

  useEffect(() => {
    if (!activeSegmentId || activeSegmentId === lastScrolledId.current) return;

    lastScrolledId.current = activeSegmentId;
    const el = document.getElementById(`sub-row-${activeSegmentId}`);
    if (el && containerRef.current) {
      el.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
  }, [activeSegmentId, containerRef]);

  return { activeSegmentId };
}
