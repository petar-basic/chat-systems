import { create } from 'zustand';
import { useMemo } from 'react';
import { useWorkspaceStore, type Message } from './workspace';

const STORAGE_KEY = 'chat_unsent';

function load(): Record<string, Message[]> {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}') as Record<string, Message[]>;
  } catch {
    return {};
  }
}

function save(byScope: Record<string, Message[]>) {
  try {
    const kept = Object.fromEntries(Object.entries(byScope).filter(([, messages]) => messages.length > 0));
    localStorage.setItem(STORAGE_KEY, JSON.stringify(kept));
  } catch {
    return;
  }
}

const delivered = new Set<string>();

interface UnsentState {
  byScope: Record<string, Message[]>;
  keep: (message: Message) => void;
  drop: (message: Pick<Message, 'id' | 'channel_id' | 'thread_parent_id'>) => void;
  deliver: (message: Pick<Message, 'id' | 'channel_id' | 'thread_parent_id'>) => void;
}

export const unsentScope = (message: Pick<Message, 'channel_id' | 'thread_parent_id'>) =>
  message.thread_parent_id ?? message.channel_id;

export const useUnsentStore = create<UnsentState>((set, get) => {
  const update = (byScope: Record<string, Message[]>) => {
    set({ byScope });
    save(byScope);
  };

  return {
    byScope: load(),
    keep: (message) => {
      if (delivered.has(message.id)) return;
      const scope = unsentScope(message);
      const byScope = get().byScope;
      const others = (byScope[scope] ?? []).filter((m) => m.id !== message.id);
      update({ ...byScope, [scope]: [...others, message] });
    },
    drop: (message) => {
      const scope = unsentScope(message);
      const byScope = get().byScope;
      const current = byScope[scope];
      if (!current?.some((m) => m.id === message.id)) return;
      update({ ...byScope, [scope]: current.filter((m) => m.id !== message.id) });
    },
    deliver: (message) => {
      delivered.add(message.id);
      get().drop(message);
    },
  };
});

const NONE: Message[] = [];

export const useUnsent = (scope: string) => {
  const userId = useWorkspaceStore((s) => s.currentUserId);
  const all = useUnsentStore((state) => state.byScope[scope] ?? NONE);
  return useMemo(() => all.filter((m) => m.user_id === userId), [all, userId]);
};

export function withUnsent(messages: Message[], unsent: Message[]): Message[] {
  if (unsent.length === 0) return messages;
  const shown = new Set(
    messages.flatMap((m) => (m.client_message_id ? [m.id, m.client_message_id] : [m.id])),
  );
  return [...messages, ...unsent.filter((m) => !shown.has(m.id))];
}
