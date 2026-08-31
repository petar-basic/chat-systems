import { splitCustomEmoji } from './customEmoji';
import type { CustomEmoji } from '@/hooks/queries/useCustomEmoji';

const PICTOGRAPH = '(?:\\p{Extended_Pictographic}\\uFE0F|\\p{Emoji_Presentation}\\uFE0F?)';
const SKIN_TONE = '[\\u{1F3FB}-\\u{1F3FF}]';
const UNIT = `${PICTOGRAPH}${SKIN_TONE}?`;
const SEQUENCE = `(?:\\p{RI}\\p{RI}|[#*0-9]\\uFE0F?\\u20E3|${UNIT}(?:\\u200D${UNIT})*)`;

const EMOJI_SEQUENCE = new RegExp(SEQUENCE, 'gu');
const HAS_PICTOGRAPH = /\p{Extended_Pictographic}|\p{RI}\p{RI}|⃣/u;

export const JUMBO_EMOJI_LIMIT = 23;

export interface TextSpan {
  text: string;
  isEmoji: boolean;
}

export function splitEmojiText(text: string): TextSpan[] {
  if (!HAS_PICTOGRAPH.test(text)) return [{ text, isEmoji: false }];

  const spans: TextSpan[] = [];
  let cursor = 0;

  for (const match of text.matchAll(EMOJI_SEQUENCE)) {
    const start = match.index ?? 0;
    if (start > cursor) spans.push({ text: text.slice(cursor, start), isEmoji: false });
    const previous = spans[spans.length - 1];
    if (previous?.isEmoji && start === cursor) previous.text += match[0];
    else spans.push({ text: match[0], isEmoji: true });
    cursor = start + match[0].length;
  }

  if (spans.length === 0) return [{ text, isEmoji: false }];
  if (cursor < text.length) spans.push({ text: text.slice(cursor), isEmoji: false });
  return spans;
}

export function countEmojiOnly(content: string, byName: Map<string, CustomEmoji>): number {
  const trimmed = content.trim();
  if (!trimmed) return 0;

  let count = 0;
  for (const span of splitCustomEmoji(trimmed, byName)) {
    if (span.emoji) {
      count += 1;
      continue;
    }
    for (const part of splitEmojiText(span.text)) {
      if (part.isEmoji) count += part.text.match(EMOJI_SEQUENCE)?.length ?? 1;
      else if (part.text.trim() !== '') return 0;
    }
  }

  return count;
}

export function isJumboEmoji(content: string, byName: Map<string, CustomEmoji>): boolean {
  const count = countEmojiOnly(content, byName);
  return count > 0 && count <= JUMBO_EMOJI_LIMIT;
}
