import { create } from 'zustand';
import { ToastKind } from '@/models/enums';
import { TOAST_DEFAULT_DURATION_MS } from '@/shared/constants';

export interface IToast {
  id: string;
  kind: ToastKind;
  message: string;
  action?: { label: string; onClick: () => void };
  durationMs: number;
}

interface IToastInput {
  kind?: ToastKind;
  action?: IToast['action'];
  durationMs?: number;
}

interface ToastState {
  toasts: IToast[];
  add: (message: string, opts?: IToastInput) => string;
  dismiss: (id: string) => void;
  clear: () => void;
}

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  add: (message, opts = {}) => {
    const id = crypto.randomUUID();
    const toast: IToast = {
      id,
      message,
      kind: opts.kind ?? ToastKind.info,
      action: opts.action,
      durationMs: opts.durationMs ?? TOAST_DEFAULT_DURATION_MS,
    };
    set((s) => ({ toasts: [...s.toasts, toast] }));
    return id;
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  clear: () => set({ toasts: [] }),
}));

export const toast = {
  success: (message: string, opts?: IToastInput) =>
    useToastStore.getState().add(message, { ...opts, kind: ToastKind.success }),
  error: (message: string, opts?: IToastInput) =>
    useToastStore.getState().add(message, { ...opts, kind: ToastKind.error }),
  info: (message: string, opts?: IToastInput) =>
    useToastStore.getState().add(message, { ...opts, kind: ToastKind.info }),
  dismiss: (id: string) => useToastStore.getState().dismiss(id),
} as const;
