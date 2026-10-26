import type { components } from '@/api/schema';

export type MessageField = components['schemas']['MessageField'];

const EMPTY = '—';
const LONG = 60;

function summaryOf(fields: MessageField[]): string {
  return fields
    .map(({ label, value }) => `**${label}**  \n${value.trim() ? value.replace(/\n/g, '  \n') : EMPTY}`)
    .join('\n\n');
}

export function introOf(content: string, fields: MessageField[]): string | null {
  if (fields.length === 0) return null;
  const summary = `\n\n${summaryOf(fields)}`;
  return content.endsWith(summary) ? content.slice(0, -summary.length).trim() : null;
}

export function isLongAnswer(value: string): boolean {
  return value.length > LONG || value.includes('\n');
}
