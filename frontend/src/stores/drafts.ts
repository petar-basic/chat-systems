import { create } from 'zustand';
import { currentUserOf, useInstanceStore } from './instances';

const STORAGE_KEY = 'chat_drafts';

export interface DraftPlace {
  instanceUrl: string;
  workspaceId: string;
  channelId: string;
  threadParentId: string | null;
}

export interface SavedDraft {
  workspace_id: string;
  channel_id: string;
  thread_parent_id?: string | null;
  content: string;
  files_left_behind: boolean;
  updated_at: string;
}

export interface LocalDraft {
  text: string;
  at: string;
  place?: DraftPlace;
  pending?: boolean;
  filesLeftBehind?: boolean;
}

export const draftKeyOf = (place: Pick<DraftPlace, 'channelId' | 'threadParentId'>) =>
  place.threadParentId ? `${place.channelId}/${place.threadParentId}` : place.channelId;

const placeOf = (instanceUrl: string, saved: SavedDraft): DraftPlace => ({
  instanceUrl,
  workspaceId: saved.workspace_id,
  channelId: saved.channel_id,
  threadParentId: saved.thread_parent_id ?? null,
});

const isLocalDraft = (value: unknown): value is LocalDraft =>
  !!value &&
  typeof value === 'object' &&
  typeof (value as LocalDraft).text === 'string' &&
  typeof (value as LocalDraft).at === 'string';

function load(): Record<string, LocalDraft> {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}') as Record<string, unknown>;
    return Object.fromEntries(Object.entries(stored).filter(([, value]) => isLocalDraft(value))) as Record<
      string,
      LocalDraft
    >;
  } catch {
    return {};
  }
}

const ownPrefix = () => `${currentUserOf(useInstanceStore.getState())?.id ?? ''}:`;
const ownKey = (key: string) => `${ownPrefix()}${key}`;
const later = (a: string, b: string) => Date.parse(a) > Date.parse(b);

export const REMOTE_DRAFT_EVENT = 'chat:remote-draft';
export const RESTORE_DRAFT_EVENT = 'chat:restore-draft';

const announceRemote = (key: string, text: string) =>
  window.dispatchEvent(new CustomEvent(REMOTE_DRAFT_EVENT, { detail: { key, text } }));

interface DraftState {
  drafts: Record<string, LocalDraft>;
  getDraft: (key: string) => string;
  entry: (key: string) => LocalDraft | undefined;
  setDraft: (key: string, value: string, extra?: { place?: DraftPlace; filesLeftBehind?: boolean }) => void;
  clearDraft: (key: string) => void;
  discardDraft: (key: string) => void;
  accept: (instanceUrl: string, saved: SavedDraft) => void;
  settle: (key: string, sentAt: string, saved: SavedDraft) => void;
  keepLocal: (key: string) => void;
  forgetMissing: (workspaceId: string, kept: Set<string>) => void;
}

export const useDraftStore = create<DraftState>((set, get) => {
  const commit = (drafts: Record<string, LocalDraft>) => {
    set({ drafts });
    localStorage.setItem(STORAGE_KEY, JSON.stringify(drafts));
  };

  const take = (key: string, place: DraftPlace, saved: SavedDraft) => {
    const id = ownKey(key);
    const before = get().drafts[id];
    const next = { ...get().drafts };
    if (saved.content) {
      next[id] = {
        text: saved.content,
        at: saved.updated_at,
        place,
        pending: false,
        filesLeftBehind: saved.files_left_behind,
      };
    } else {
      delete next[id];
    }
    commit(next);
    if ((before?.text ?? '') !== saved.content) announceRemote(key, saved.content);
  };

  return {
    drafts: load(),
    getDraft: (key) => get().drafts[ownKey(key)]?.text ?? '',
    entry: (key) => get().drafts[ownKey(key)],
    setDraft: (key, value, extra = {}) => {
      const id = ownKey(key);
      const current = get().drafts[id];
      const place = extra.place ?? current?.place;
      const text = value.trim() ? value : '';
      const filesLeftBehind = !!text && (extra.filesLeftBehind ?? current?.filesLeftBehind ?? false);
      if (!current && !text) return;
      const unchanged = current && current.text === text && !!current.filesLeftBehind === filesLeftBehind;
      if (unchanged && (current.place || !place)) return;
      const next = { ...get().drafts };
      if (!text && !place) delete next[id];
      else next[id] = { text, at: new Date().toISOString(), place, pending: !!place, filesLeftBehind };
      commit(next);
    },
    clearDraft: (key) => get().setDraft(key, ''),
    discardDraft: (key) => {
      get().setDraft(key, '');
      announceRemote(key, '');
    },
    accept: (instanceUrl, saved) => {
      const place = placeOf(instanceUrl, saved);
      const key = draftKeyOf(place);
      const current = get().drafts[ownKey(key)];
      if (current && later(current.at, saved.updated_at)) return;
      if (!current && !saved.content) return;
      take(key, place, saved);
    },
    settle: (key, sentAt, saved) => {
      const current = get().drafts[ownKey(key)];
      if (!current?.place || Date.parse(current.at) !== Date.parse(sentAt)) return;
      take(key, current.place, saved);
    },
    keepLocal: (key) => {
      const id = ownKey(key);
      const current = get().drafts[id];
      if (!current) return;
      const next = { ...get().drafts };
      if (current.text) next[id] = { text: current.text, at: current.at };
      else delete next[id];
      commit(next);
    },
    forgetMissing: (workspaceId, kept) => {
      const prefix = ownPrefix();
      const gone = Object.entries(get().drafts).filter(
        ([id, draft]) =>
          id.startsWith(prefix) &&
          draft.place?.workspaceId === workspaceId &&
          !draft.pending &&
          !kept.has(id.slice(prefix.length)),
      );
      if (gone.length === 0) return;
      const next = { ...get().drafts };
      for (const [id] of gone) delete next[id];
      commit(next);
      for (const [id] of gone) announceRemote(id.slice(prefix.length), '');
    },
  };
});

export function restoreDraft(key: string, text: string) {
  const current = useDraftStore.getState().getDraft(key).trim();
  const next = current ? `${current}\n\n${text}` : text;
  useDraftStore.getState().setDraft(key, next);
  window.dispatchEvent(new CustomEvent(RESTORE_DRAFT_EVENT, { detail: { key, text: next } }));
}

export function ownDrafts(drafts: Record<string, LocalDraft>): [string, LocalDraft][] {
  const prefix = ownPrefix();
  return Object.entries(drafts)
    .filter(([id]) => id.startsWith(prefix))
    .map(([id, draft]) => [id.slice(prefix.length), draft]);
}

export function hasDraftIn(drafts: Record<string, LocalDraft>, channelId: string): boolean {
  const prefix = ownPrefix();
  return !!drafts[`${prefix}${channelId}`]?.text;
}
