import { useEffect } from 'react';
import { CheckCircle2, AlertCircle, Info, X } from 'lucide-react';
import { ToastKind } from '@/models/enums';
import { useToastStore, type IToast } from './toast-store';

const KIND_STYLES: Record<ToastKind, { ring: string; icon: React.ReactNode }> = {
  [ToastKind.success]: {
    ring: 'border-green-500/40',
    icon: <CheckCircle2 className="w-4 h-4 text-green-400" />,
  },
  [ToastKind.error]: {
    ring: 'border-red-500/40',
    icon: <AlertCircle className="w-4 h-4 text-red-400" />,
  },
  [ToastKind.info]: {
    ring: 'border-slate-500/40',
    icon: <Info className="w-4 h-4 text-slate-300" />,
  },
};

function ToastRow({ toast }: { toast: IToast }) {
  const dismiss = useToastStore((s) => s.dismiss);

  useEffect(() => {
    if (toast.durationMs <= 0) return;
    const timer = setTimeout(() => dismiss(toast.id), toast.durationMs);
    return () => clearTimeout(timer);
  }, [toast.id, toast.durationMs, dismiss]);

  const style = KIND_STYLES[toast.kind];

  return (
    <div
      role="status"
      data-qa={`toast-${toast.kind}`}
      className={`pointer-events-auto flex items-start gap-2.5 w-80 max-w-[90vw] bg-slate-800 border ${style.ring} rounded-xl px-3.5 py-3 shadow-2xl`}
    >
      <div className="mt-0.5 shrink-0">{style.icon}</div>
      <p className="flex-1 text-sm text-slate-200 break-words">{toast.message}</p>
      {toast.action && (
        <button
          onClick={() => {
            toast.action!.onClick();
            dismiss(toast.id);
          }}
          className="shrink-0 text-xs font-semibold text-purple-300 hover:text-purple-200"
        >
          {toast.action.label}
        </button>
      )}
      <button
        onClick={() => dismiss(toast.id)}
        aria-label="Dismiss notification"
        className="shrink-0 text-slate-400 hover:text-slate-200"
      >
        <X className="w-3.5 h-3.5" />
      </button>
    </div>
  );
}

export function Toaster() {
  const toasts = useToastStore((s) => s.toasts);
  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-[100] flex flex-col gap-2 pointer-events-none">
      {toasts.map((t) => (
        <ToastRow key={t.id} toast={t} />
      ))}
    </div>
  );
}
