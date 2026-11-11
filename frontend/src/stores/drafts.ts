import { create } from 'zustand';
import { currentUserOf, useInstanceStore } from './instances';

const STORAGE_KEY = 'chat_drafts';

function load(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}') as Record<string, string>;
  } catch {
    return {};
  }
}

function ownKey(place: string): string {
  return `${currentUserOf(useInstanceStore.getState())?.id ?? ''}:${place}`;
}

interface DraftState {
  drafts: Record<string, string>;
  getDraft: (place: string) => string;
  setDraft: (place: string, value: string) => void;
  clearDraft: (place: string) => void;
}

export const useDraftStore = create<DraftState>((set, get) => {
  const persist = (drafts: Record<string, string>) =>
    localStorage.setItem(STORAGE_KEY, JSON.stringify(drafts));

  return {
    drafts: load(),
    getDraft: (place) => get().drafts[ownKey(place)] ?? '',
    setDraft: (place, value) => {
      const key = ownKey(place);
      const next = { ...get().drafts };
      if (value.trim()) next[key] = value;
      else delete next[key];
      set({ drafts: next });
      persist(next);
    },
    clearDraft: (place) => {
      const key = ownKey(place);
      if (!(key in get().drafts)) return;
      const next = { ...get().drafts };
      delete next[key];
      set({ drafts: next });
      persist(next);
    },
  };
});

export const RESTORE_DRAFT_EVENT = 'chat:restore-draft';

export function restoreDraft(key: string, text: string) {
  const current = useDraftStore.getState().getDraft(key).trim();
  const next = current ? `${current}\n\n${text}` : text;
  useDraftStore.getState().setDraft(key, next);
  window.dispatchEvent(new CustomEvent(RESTORE_DRAFT_EVENT, { detail: { key, text: next } }));
}
