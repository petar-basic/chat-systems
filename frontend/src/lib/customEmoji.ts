import type { CustomEmoji } from '@/hooks/queries/useCustomEmoji';

export type EmojiSpan = { text: string; emoji: CustomEmoji | null };

const SHORTCODE = /:([a-z0-9_-]{2,32}):/g;

/**
 * Splits text around `:name:` runs that a workspace has an image for. Standard
 * shortcodes never reach here — the composer has already turned them into the
 * Unicode character, and the upload path refuses a custom name that would
 * shadow one.
 */
export function splitCustomEmoji(text: string, byName: Map<string, CustomEmoji>): EmojiSpan[] {
  if (byName.size === 0 || !text.includes(':')) return [{ text, emoji: null }];

  const spans: EmojiSpan[] = [];
  let cursor = 0;

  for (const match of text.matchAll(SHORTCODE)) {
    const emoji = byName.get(match[1]);
    if (!emoji) continue;
    const start = match.index ?? 0;
    if (start > cursor) spans.push({ text: text.slice(cursor, start), emoji: null });
    spans.push({ text: match[0], emoji });
    cursor = start + match[0].length;
  }

  if (spans.length === 0) return [{ text, emoji: null }];
  if (cursor < text.length) spans.push({ text: text.slice(cursor), emoji: null });
  return spans;
}
