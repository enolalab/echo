import React, { useCallback, useRef, useState } from 'react';

interface SplitPaneProps {
  direction: 'horizontal' | 'vertical';
  initialRatio: number;
  minA: number;
  minB: number;
  storageKey?: string;
  children: [React.ReactNode, React.ReactNode];
}

function loadRatio(storageKey: string | undefined, fallback: number): number {
  if (!storageKey) return fallback;
  try {
    const raw = localStorage.getItem(`echo.layout.${storageKey}`);
    const v = raw === null ? NaN : Number(raw);
    return Number.isFinite(v) && v > 0 && v < 1 ? v : fallback;
  } catch {
    return fallback;
  }
}

export const SplitPane: React.FC<SplitPaneProps> = ({
  direction,
  initialRatio,
  minA,
  minB,
  storageKey,
  children,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [ratio, setRatio] = useState(() => loadRatio(storageKey, initialRatio));
  const horizontal = direction === 'horizontal';

  const startDrag = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const el = containerRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const size = horizontal ? rect.width : rect.height;
      if (size <= 0) return;
      const start = horizontal ? rect.left : rect.top;

      document.body.style.userSelect = 'none';
      document.body.style.cursor = horizontal ? 'col-resize' : 'row-resize';

      const onMove = (ev: MouseEvent) => {
        const pos = (horizontal ? ev.clientX : ev.clientY) - start;
        const minR = minA / size;
        const maxR = 1 - minB / size;
        setRatio(Math.min(Math.max(pos / size, minR), Math.max(minR, maxR)));
      };
      const onUp = () => {
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
        document.body.style.userSelect = '';
        document.body.style.cursor = '';
        if (storageKey) {
          setRatio((r) => {
            try {
              localStorage.setItem(`echo.layout.${storageKey}`, String(r));
            } catch {
              /* ignore */
            }
            return r;
          });
        }
      };
      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    },
    [horizontal, minA, minB, storageKey]
  );

  return (
    <div
      ref={containerRef}
      className={`flex ${horizontal ? 'flex-row' : 'flex-col'} h-full w-full min-h-0 min-w-0`}
    >
      <div
        data-testid="split-a"
        style={{ flexBasis: `${ratio * 100}%` }}
        className="shrink-0 min-w-0 min-h-0 overflow-hidden"
      >
        {children[0]}
      </div>
      <div
        role="separator"
        aria-orientation={horizontal ? 'vertical' : 'horizontal'}
        onMouseDown={startDrag}
        className={`shrink-0 ${
          horizontal ? 'w-[5px] cursor-col-resize' : 'h-[5px] cursor-row-resize'
        } bg-slate-800 hover:bg-blue-600/50 transition-colors`}
      />
      <div className="flex-1 min-w-0 min-h-0 overflow-hidden">{children[1]}</div>
    </div>
  );
};
