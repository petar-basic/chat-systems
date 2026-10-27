import { create } from 'zustand';
import type { Message } from './workspace';

interface UnsentState {
  byScope: Record<string, Message[]>;
  keep: (message: Message) => void;
  drop: (message: Pick<Message, 'id' | 'channel_id' | 'thread_parent_id'>) => void;
}

export const unsentScope = (message: Pick<Message, 'channel_id' | 'thread_parent_id'>) =>
  message.thread_parent_id ?? message.channel_id;

export const useUnsentStore = create<UnsentState>((set) => ({
  byScope: {},
  keep: (message) =>
    set((state) => {
      const scope = unsentScope(message);
      const others = (state.byScope[scope] ?? []).filter((m) => m.id !== message.id);
      return { byScope: { ...state.byScope, [scope]: [...others, message] } };
    }),
  drop: (message) =>
    set((state) => {
      const scope = unsentScope(message);
      const current = state.byScope[scope];
      if (!current?.some((m) => m.id === message.id)) return state;
      return { byScope: { ...state.byScope, [scope]: current.filter((m) => m.id !== message.id) } };
    }),
}));

const NONE: Message[] = [];

export const useUnsent = (scope: string) => useUnsentStore((state) => state.byScope[scope] ?? NONE);

export function withUnsent(messages: Message[], unsent: Message[]): Message[] {
  if (unsent.length === 0) return messages;
  const shown = new Set(messages.map((m) => m.id));
  return [...messages, ...unsent.filter((m) => !shown.has(m.id))];
}
