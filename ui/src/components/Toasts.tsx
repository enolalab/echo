import React from 'react';
import { X, CheckCircle2, Info, AlertTriangle } from 'lucide-react';

export interface Toast {
  id: number;
  text: string;
  kind: 'info' | 'success' | 'error';
}

const KIND_STYLE: Record<Toast['kind'], string> = {
  info: 'bg-slate-800 border-slate-600 text-slate-200',
  success: 'bg-emerald-900/90 border-emerald-700 text-emerald-200',
  error: 'bg-rose-900/90 border-rose-700 text-rose-200',
};

const KIND_ICON: Record<Toast['kind'], React.ReactNode> = {
  info: <Info className="w-3.5 h-3.5 shrink-0" />,
  success: <CheckCircle2 className="w-3.5 h-3.5 shrink-0" />,
  error: <AlertTriangle className="w-3.5 h-3.5 shrink-0" />,
};

export const Toasts: React.FC<{ toasts: Toast[]; onDismiss: (id: number) => void }> = ({
  toasts,
  onDismiss,
}) => (
  <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
    {toasts.map((t) => (
      <div
        key={t.id}
        className={`flex items-start gap-2 rounded-md border px-3 py-2 text-xs shadow-lg ${KIND_STYLE[t.kind]}`}
      >
        {KIND_ICON[t.kind]}
        <span className="flex-1">{t.text}</span>
        <button
          aria-label="Đóng thông báo"
          onClick={() => onDismiss(t.id)}
          className="shrink-0 opacity-70 hover:opacity-100"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
    ))}
  </div>
);
