import type { MentionRef } from './mentionHighlightExtension';

const MENTION_TOKEN = /@\[([^\]]+)\]\(([^)]+)\)/g;

export function parseMentions(content: string): MentionRef[] {
  const out: MentionRef[] = [];
  MENTION_TOKEN.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = MENTION_TOKEN.exec(content)) !== null) {
    out.push({ label: match[1], id: match[2] });
  }
  return out;
}

export function flattenMentions(content: string): string {
  return content.replace(MENTION_TOKEN, (_m, label) => `@${label}`);
}
