import { create } from 'zustand';
import type { CustomEmoji } from '@/hooks/queries/useCustomEmoji';

interface CustomEmojiState {
  byName: Map<string, CustomEmoji>;
  populate: (emojis: CustomEmoji[]) => void;
}

/**
 * A store rather than a query hook inside the renderer. `MessageContent` renders
 * once per message and is deliberately free of server state — a `useQuery` there
 * would also make every test of it need a QueryClient to render text.
 */
export const useCustomEmojiStore = create<CustomEmojiState>((set) => ({
  byName: new Map(),
  populate: (emojis) => set({ byName: new Map(emojis.map((e) => [e.name, e])) }),
}));
