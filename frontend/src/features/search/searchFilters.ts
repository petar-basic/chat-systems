import { displayNameOf } from '@/lib/userHelpers';
import { FILE_KINDS, type FileKindName } from '@/lib/fileKinds';

export type HasFilter = 'file' | 'image' | 'link';

export type SearchTab = 'messages' | 'files';

export interface SearchFilters {
  from?: string;
  in?: string;
  has?: HasFilter;
  kind?: FileKindName;
  on?: string;
  after?: string;
  before?: string;
}

export interface FilterContext {
  members: { user_id: string; display_name?: string | null }[];
  channels: { id: string; name?: string | null; channel_type?: string | null }[];
  conversations: { id: string; kind: string; participant_ids: string[] }[];
  currentUserId?: string | null;
  today: Date;
}

export interface SearchParams {
  user_id?: string;
  channel_id?: string;
  has?: HasFilter;
  kind?: FileKindName;
  after?: string;
  before?: string;
}

const MODIFIER = /^(from|in|has|type|on|before|after):(.+)$/i;

const HAS: Record<string, HasFilter> = {
  file: 'file',
  files: 'file',
  image: 'image',
  images: 'image',
  link: 'link',
  links: 'link',
};

const KINDS = new Map(
  (Object.keys(FILE_KINDS) as FileKindName[]).flatMap((kind) => [
    [kind, kind],
    [`${kind}s`, kind],
  ]),
);

export const HAS_LABELS: Record<HasFilter, string> = { file: 'Files', image: 'Images', link: 'Links' };

const squash = (value: string) => value.toLowerCase().replace(/\s+/g, '');

export function dayKey(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function parseDay(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return null;
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const date = new Date(year, month - 1, day);
  return date.getMonth() === month - 1 && date.getDate() === day ? date : null;
}

export function shiftDay(key: string, days: number): string {
  const date = parseDay(key) ?? new Date(NaN);
  return dayKey(new Date(date.getFullYear(), date.getMonth(), date.getDate() + days));
}

function resolveDay(value: string, today: Date): string | null {
  const word = value.toLowerCase();
  if (word === 'today') return dayKey(today);
  if (word === 'yesterday') return shiftDay(dayKey(today), -1);
  const date = parseDay(value);
  return date ? dayKey(date) : null;
}

export function findMember(value: string, ctx: FilterContext): string | null {
  const wanted = squash(value.replace(/^@/, ''));
  if (!wanted) return null;
  if (wanted === 'me') return ctx.currentUserId ?? null;
  const named = ctx.members.map((m) => ({ id: m.user_id, name: squash(m.display_name ?? '') }));
  const exact = named.find((m) => m.name === wanted);
  if (exact) return exact.id;
  const prefixed = named.filter((m) => m.name.startsWith(wanted));
  return prefixed.length === 1 ? prefixed[0].id : null;
}

export function directMessageWith(userId: string, ctx: FilterContext): string | null {
  const conversation = ctx.conversations.find(
    (c) =>
      c.kind === 'dm' &&
      c.participant_ids.includes(userId) &&
      c.participant_ids.every((id) => id === userId || id === ctx.currentUserId),
  );
  return conversation?.id ?? null;
}

function resolvePlace(value: string, ctx: FilterContext): string | null {
  if (value.startsWith('@')) {
    const member = findMember(value, ctx);
    return member ? directMessageWith(member, ctx) : null;
  }
  const wanted = value.replace(/^#/, '').toLowerCase();
  return ctx.channels.find((c) => (c.name ?? '').toLowerCase() === wanted)?.id ?? null;
}

function resolve(key: string, value: string, ctx: FilterContext): SearchFilters | null {
  switch (key) {
    case 'from': {
      const member = findMember(value, ctx);
      return member ? { from: member } : null;
    }
    case 'in': {
      const place = resolvePlace(value, ctx);
      return place ? { in: place } : null;
    }
    case 'has': {
      const has = HAS[value.toLowerCase()];
      return has ? { has } : null;
    }
    case 'type': {
      const kind = KINDS.get(value.toLowerCase());
      return kind ? { kind } : null;
    }
    default: {
      const day = resolveDay(value, ctx.today);
      if (!day) return null;
      if (key === 'before') return { before: day, on: undefined };
      if (key === 'after') return { after: day, on: undefined };
      return { on: day, after: undefined, before: undefined };
    }
  }
}

export function extractFilters(
  input: string,
  ctx: FilterContext,
  final: boolean,
): { text: string; filters: SearchFilters } | null {
  const tokens = input.split(/\s+/).filter(Boolean);
  const endsOpen = !final && !/\s$/.test(input);
  const kept: string[] = [];
  let filters: SearchFilters = {};
  let extracted = false;
  for (const [index, token] of tokens.entries()) {
    const match = MODIFIER.exec(token);
    const complete = !(endsOpen && index === tokens.length - 1);
    const found = match && complete ? resolve(match[1].toLowerCase(), match[2], ctx) : null;
    if (found) {
      filters = { ...filters, ...found };
      extracted = true;
    } else {
      kept.push(token);
    }
  }
  if (!extracted) return null;
  const text = kept.join(' ');
  return { text: text && /\s$/.test(input) ? `${text} ` : text, filters };
}

export function setDate(
  filters: SearchFilters,
  date: Pick<SearchFilters, 'on' | 'after' | 'before'>,
): SearchFilters {
  if (date.on) return { ...filters, on: date.on, after: undefined, before: undefined };
  return { ...filters, on: undefined, ...date };
}

export function kindLabel(kind: FileKindName): string {
  return kind === 'other' ? 'Other' : FILE_KINDS[kind].label;
}

export function forTab(filters: SearchFilters, tab: SearchTab): SearchFilters {
  const { has, kind, ...shared } = filters;
  return tab === 'files' ? { ...shared, kind } : { ...shared, has };
}

export function hasFilters(filters: SearchFilters): boolean {
  return Object.values(filters).some(Boolean);
}

const startOf = (key: string) => (parseDay(key) ?? new Date(NaN)).toISOString();

export function searchParams(filters: SearchFilters): SearchParams {
  const params: SearchParams = {};
  if (filters.from) params.user_id = filters.from;
  if (filters.in) params.channel_id = filters.in;
  if (filters.has) params.has = filters.has;
  if (filters.kind) params.kind = filters.kind;
  if (filters.on) {
    params.after = startOf(filters.on);
    params.before = startOf(shiftDay(filters.on, 1));
  } else {
    if (filters.after) params.after = startOf(shiftDay(filters.after, 1));
    if (filters.before) params.before = startOf(filters.before);
  }
  return params;
}

function formatDay(key: string): string {
  return (parseDay(key) ?? new Date(NaN)).toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

export function dateLabel(filters: SearchFilters): string | null {
  if (filters.on) return `On ${formatDay(filters.on)}`;
  const parts = [
    filters.after && `After ${formatDay(filters.after)}`,
    filters.before && `${filters.after ? 'before' : 'Before'} ${formatDay(filters.before)}`,
  ].filter(Boolean);
  return parts.length ? parts.join(', ') : null;
}

export function memberName(ctx: FilterContext, userId: string): string {
  if (userId === ctx.currentUserId) return 'You';
  return displayNameOf(ctx.members.find((m) => m.user_id === userId)?.display_name);
}

export function placeName(ctx: FilterContext, placeId: string): string {
  const channel = ctx.channels.find((c) => c.id === placeId);
  if (channel) return `#${channel.name ?? 'channel'}`;
  const conversation = ctx.conversations.find((c) => c.id === placeId);
  if (!conversation) return 'Conversation';
  const others = conversation.participant_ids.filter((id) => id !== ctx.currentUserId);
  return others.length ? others.map((id) => memberName(ctx, id)).join(', ') : 'You';
}
