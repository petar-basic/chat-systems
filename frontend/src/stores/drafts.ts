import { create } from 'zustand';

const STORAGE_KEY = 'chat_drafts';

function load(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}') as Record<string, string>;
  } catch {
    return {};
  }
}

interface DraftState {
  drafts: Record<string, string>;
  getDraft: (key: string) => string;
  setDraft: (key: string, value: string) => void;
  clearDraft: (key: string) => void;
}

export const useDraftStore = create<DraftState>((set, get) => {
  const persist = (drafts: Record<string, string>) =>
    localStorage.setItem(STORAGE_KEY, JSON.stringify(drafts));

  return {
    drafts: load(),
    getDraft: (key) => get().drafts[key] ?? '',
    setDraft: (key, value) => {
      const next = { ...get().drafts };
      if (value.trim()) next[key] = value;
      else delete next[key];
      set({ drafts: next });
      persist(next);
    },
    clearDraft: (key) => {
      if (!(key in get().drafts)) return;
      const next = { ...get().drafts };
      delete next[key];
      set({ drafts: next });
      persist(next);
    },
  };
});
