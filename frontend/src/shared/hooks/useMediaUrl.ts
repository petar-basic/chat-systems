import { useCallback, useMemo, useState } from 'react';
import { mediaUrl } from '@/lib/mediaUrl';
import type { CustomEmoji } from '@/features/workspace/hooks/useCustomEmoji';
import { useCustomEmojiStore } from '@/stores/customEmoji';
import { useMediaTokenStore } from '@/stores/mediaTokens';

export function useMediaLinks(): (src: string) => string {
  const tokens = useMediaTokenStore((s) => s.tokens);
  return useCallback((src: string) => mediaUrl(src, tokens), [tokens]);
}

export function useMediaUrl<T extends string | null | undefined>(src: T): T | string {
  const link = useMediaLinks();
  return src ? link(src) : src;
}

export function useMediaSource(src: string): { src: string; onError: () => void } {
  const live = useMediaUrl(src);
  const [held, setHeld] = useState(live);
  return { src: held, onError: () => setHeld(live) };
}

export function useLinkedCustomEmoji(): Map<string, CustomEmoji> {
  const byName = useCustomEmojiStore((s) => s.byName);
  const link = useMediaLinks();
  return useMemo(
    () => new Map([...byName].map(([name, emoji]) => [name, { ...emoji, url: link(emoji.url) }])),
    [byName, link],
  );
}
