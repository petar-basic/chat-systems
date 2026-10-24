import type { components } from '@/api/schema';

export type List = components['schemas']['List'];
export type ListColumn = components['schemas']['ListColumn'];
export type ListItem = components['schemas']['ListItem'];
export type ListDetail = components['schemas']['ListDetail'];
export type ColumnKind = components['schemas']['ColumnKind'];
export type Choice = components['schemas']['Choice'];
export type ChoiceColor = components['schemas']['ChoiceColor'];
export type NewColumn = components['schemas']['NewColumn'];
export type CellValues = ListItem['values'];
export type ListComment = components['schemas']['ListComment'];

export const KIND_LABELS: Record<ColumnKind, string> = {
  text: 'Text',
  number: 'Number',
  select: 'Select',
  multi_select: 'Multi-select',
  person: 'Person',
  date: 'Date',
  checkbox: 'Checkbox',
  link: 'Link',
};

export const CHOICE_COLORS: ChoiceColor[] = [
  'gray',
  'red',
  'orange',
  'yellow',
  'green',
  'blue',
  'purple',
  'pink',
];

export const CHOICE_STYLES: Record<ChoiceColor, string> = {
  gray: 'bg-raised text-fg-dim',
  red: 'bg-red-500/15 text-red-400',
  orange: 'bg-orange-500/15 text-orange-400',
  yellow: 'bg-yellow-500/15 text-yellow-400',
  green: 'bg-green-500/15 text-green-400',
  blue: 'bg-blue-500/15 text-blue-400',
  purple: 'bg-purple-500/15 text-purple-400',
  pink: 'bg-pink-500/15 text-pink-400',
};

export const COLUMN_WIDTHS: Record<ColumnKind, number> = {
  text: 240,
  number: 120,
  select: 160,
  multi_select: 220,
  person: 180,
  date: 150,
  checkbox: 96,
  link: 220,
};

export const MAX_COLUMNS = 30;
export const MAX_ITEMS = 10_000;

export const hasChoices = (kind: ColumnKind) => kind === 'select' || kind === 'multi_select';
export const typesText = (kind: ColumnKind) => kind === 'text' || kind === 'number' || kind === 'link';

export function byRank(a: ListItem, b: ListItem): number {
  if (a.rank !== b.rank) return a.rank < b.rank ? -1 : 1;
  if (a.id === b.id) return 0;
  return a.id < b.id ? -1 : 1;
}

export function upsertItem(items: ListItem[], item: ListItem): ListItem[] {
  const existing = items.find((i) => i.id === item.id);
  if (existing && existing.updated_at > item.updated_at) return items;
  const rest = existing ? items.filter((i) => i.id !== item.id) : items;
  const at = rest.findIndex((i) => byRank(item, i) < 0);
  if (at === -1) return [...rest, item];
  return [...rest.slice(0, at), item, ...rest.slice(at)];
}

export const removeItem = (items: ListItem[], itemId: string) => items.filter((i) => i.id !== itemId);

export function upsertComment(comments: ListComment[], comment: ListComment): ListComment[] {
  const rest = comments.filter((c) => c.id !== comment.id);
  return [...rest, comment].sort((a, b) =>
    a.created_at === b.created_at ? a.id.localeCompare(b.id) : a.created_at < b.created_at ? -1 : 1,
  );
}

export const removeComment = (comments: ListComment[], commentId: string) =>
  comments.filter((c) => c.id !== commentId);

export function withCells(item: ListItem, cells: CellValues): ListItem {
  const values: CellValues = { ...item.values };
  for (const [columnId, value] of Object.entries(cells)) {
    if (value === null) delete values[columnId];
    else values[columnId] = value;
  }
  return { ...item, values };
}

export function withoutColumn(items: ListItem[], columnId: string): ListItem[] {
  return items.map((item) => {
    if (!(columnId in item.values)) return item;
    const values: CellValues = { ...item.values };
    delete values[columnId];
    return { ...item, values };
  });
}

const DIGITS = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';

const digit = (char: string | undefined) => (char === undefined ? 0 : Math.max(0, DIGITS.indexOf(char)));

function midpoint(low: string, high: string | null): string {
  if (high !== null) {
    let shared = 0;
    while (shared < high.length && (low[shared] ?? '0') === high[shared]) shared += 1;
    if (shared > 0) return high.slice(0, shared) + midpoint(low.slice(shared), high.slice(shared));
  }
  const lowDigit = digit(low[0]);
  const highDigit = high !== null && high.length > 0 ? digit(high[0]) : DIGITS.length;
  if (highDigit - lowDigit > 1) return DIGITS[Math.ceil((lowDigit + highDigit) / 2)];
  if (high !== null && high.length > 1) return high[0];
  return DIGITS[lowDigit] + midpoint(low.slice(1), null);
}

export const rankBetween = (before: string | null, after: string | null) => midpoint(before ?? '', after);

export function movedRank(items: ListItem[], itemId: string, afterId: string | null): string {
  const rest = items.filter((i) => i.id !== itemId);
  const at = afterId === null ? -1 : rest.findIndex((i) => i.id === afterId);
  const before = at === -1 ? null : rest[at].rank;
  const after = rest[at + 1]?.rank ?? null;
  return rankBetween(before, after);
}

export function choiceOf(column: ListColumn, id: unknown): Choice | undefined {
  return typeof id === 'string' ? column.options.choices?.find((choice) => choice.id === id) : undefined;
}

export function formatNumber(column: ListColumn, value: number): string {
  const format = column.options.number;
  const decimals = format?.decimals ?? 0;
  const digits = { minimumFractionDigits: decimals, maximumFractionDigits: Math.max(decimals, 6) };
  if (format?.style === 'currency' && format.currency) {
    return new Intl.NumberFormat(undefined, {
      style: 'currency',
      currency: format.currency,
      ...digits,
    }).format(value);
  }
  const text = new Intl.NumberFormat(undefined, digits).format(value);
  return format?.style === 'percent' ? `${text}%` : text;
}

export function formatDate(value: string): string {
  const [year, month, day] = value.split('-').map(Number);
  if (!year || !month || !day) return value;
  return new Date(year, month - 1, day).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

export function cellText(column: ListColumn, value: unknown, nameOf: (userId: string) => string): string {
  if (value === undefined || value === null) return '';
  switch (column.kind) {
    case 'number':
      return typeof value === 'number' ? formatNumber(column, value) : String(value);
    case 'select':
      return choiceOf(column, value)?.label ?? '';
    case 'multi_select':
      return Array.isArray(value)
        ? value
            .map((id) => choiceOf(column, id)?.label)
            .filter(Boolean)
            .join(', ')
        : '';
    case 'person':
      return typeof value === 'string' ? nameOf(value) : '';
    case 'date':
      return typeof value === 'string' ? formatDate(value) : '';
    case 'checkbox':
      return value === true ? 'Yes' : 'No';
    default:
      return typeof value === 'string' ? value : String(value);
  }
}

export function draftOf(column: ListColumn, value: unknown): string {
  if (value === undefined || value === null) return '';
  if (column.kind === 'number' && typeof value === 'number') return String(value);
  return typeof value === 'string' ? value : '';
}

export type Parsed = { value: unknown } | { error: string };

export function parseDraft(kind: ColumnKind, draft: string): Parsed {
  const text = draft.trim();
  if (text === '') return { value: null };
  switch (kind) {
    case 'number': {
      const n = Number(text.replace(/,/g, ''));
      return Number.isFinite(n) ? { value: n } : { error: 'Enter a number' };
    }
    case 'link':
      return { value: /^https?:\/\//i.test(text) ? text : `https://${text}` };
    case 'date':
      return /^\d{4}-\d{2}-\d{2}$/.test(text) ? { value: text } : { error: 'Pick a date' };
    default:
      return { value: draft };
  }
}
