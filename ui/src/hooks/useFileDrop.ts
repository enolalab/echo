import { useEffect, useRef, useState } from 'react';
import { getCurrentWindow, type UnlistenFn } from '@tauri-apps/api/window';
import { isTauri } from '../lib/ipc';

export const VALID_EXTENSIONS = [
  'mp4', 'mkv', 'mov', 'avi', 'webm', 'm4v',
  'mp3', 'wav', 'flac', 'm4a', 'ogg',
];

export function hasValidExtension(name: string): boolean {
  const idx = name.lastIndexOf('.');
  if (idx < 0) return false;
  return VALID_EXTENSIONS.includes(name.slice(idx + 1).toLowerCase());
}

export function useFileDrop(onDrop: (paths: string[]) => void): { isDragging: boolean } {
  const [isDragging, setIsDragging] = useState(false);
  const onDropRef = useRef(onDrop);
  onDropRef.current = onDrop;

  useEffect(() => {
    if (!isTauri()) {
      const onDragOver = (e: Event) => {
        e.preventDefault();
        setIsDragging(true);
      };
      const onDragLeave = (e: Event) => {
        if (!(e as DragEvent).relatedTarget) setIsDragging(false);
      };
      const onDropEvent = (e: Event) => {
        e.preventDefault();
        setIsDragging(false);
        const files = Array.from((e as DragEvent).dataTransfer?.files ?? []).filter((f) =>
          hasValidExtension(f.name)
        );
        onDropRef.current(files.map((f) => URL.createObjectURL(f)));
      };
      document.addEventListener('dragover', onDragOver);
      document.addEventListener('dragleave', onDragLeave);
      document.addEventListener('drop', onDropEvent);
      return () => {
        document.removeEventListener('dragover', onDragOver);
        document.removeEventListener('dragleave', onDragLeave);
        document.removeEventListener('drop', onDropEvent);
      };
    }

    let unlisten: UnlistenFn | undefined;
    let cancelled = false;
    getCurrentWindow()
      .onDragDropEvent((e) => {
        const type = e.payload.type;
        if (type === 'enter' || type === 'over') setIsDragging(true);
        else if (type === 'leave') setIsDragging(false);
        else if (type === 'drop') {
          setIsDragging(false);
          onDropRef.current(e.payload.paths.filter(hasValidExtension));
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => console.warn('onDragDropEvent failed:', err));
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  return { isDragging };
}
