import { useCallback, useRef, useState } from 'react';
import type { Toast } from '../components/Toasts';

const MAX_TOASTS = 4;
const AUTO_DISMISS_MS = 4000;

export function useToasts() {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const idRef = useRef(0);
  const timers = useRef(new Map<number, ReturnType<typeof setTimeout>>());

  const dismissToast = useCallback((id: number) => {
    const t = timers.current.get(id);
    if (t) {
      clearTimeout(t);
      timers.current.delete(id);
    }
    setToasts((prev) => prev.filter((x) => x.id !== id));
  }, []);

  const pushToast = useCallback(
    (text: string, kind: Toast['kind'] = 'info') => {
      const id = ++idRef.current;
      setToasts((prev) => [...prev.slice(-(MAX_TOASTS - 1)), { id, text, kind }]);
      timers.current.set(id, setTimeout(() => dismissToast(id), AUTO_DISMISS_MS));
    },
    [dismissToast]
  );

  return { toasts, pushToast, dismissToast };
}
