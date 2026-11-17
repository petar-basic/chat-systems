import type { Message } from '@/stores/workspace';

export function safeLink(url: string | null | undefined): string | null {
  if (!url) return null;
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? parsed.href : null;
  } catch {
    return null;
  }
}

const HEX = /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i;

export function safeColour(color: string | null | undefined): string | null {
  return color && HEX.test(color) ? color : null;
}

export function hasBlocks(message: Message): boolean {
  return !!message.metadata?.blocks?.length || !!message.metadata?.legacy_attachments?.length;
}
