import type { components } from '@/api/schema';
import { byRank, choiceOf, type ColumnKind, type ListColumn, type ListItem } from './listModel';

export type SavedView = components['schemas']['ListView'];
export type ViewKind = components['schemas']['ViewKind'];
export type ViewConfig = components['schemas']['ViewConfig'];
export type SortRule = components['schemas']['SortRule'];
export type FilterRule = components['schemas']['FilterRule'];
export type FilterOp = components['schemas']['FilterOp'];

export const MAX_VIEWS = 20;
export const MAX_SORTS = 3;
export const MAX_FILTERS = 10;
export const ME = 'me';

export const OPS_BY_KIND: Record<ColumnKind, FilterOp[]> = {
  text: ['contains', 'empty', 'not_empty'],
  link: ['contains', 'empty', 'not_empty'],
  number: ['gt', 'lt', 'empty', 'not_empty'],
  select: ['is', 'is_not', 'empty', 'not_empty'],
  multi_select: ['is', 'is_not', 'empty', 'not_empty'],
  person: ['is', 'is_not', 'empty', 'not_empty'],
  date: ['this_week', 'overdue', 'lt', 'gt', 'empty', 'not_empty'],
  checkbox: ['checked', 'unchecked'],
};

export function opLabel(op: FilterOp, kind: ColumnKind): string {
  switch (op) {
    case 'contains':
      return 'contains';
    case 'is':
      return kind === 'multi_select' ? 'has' : 'is';
    case 'is_not':
      return kind === 'multi_select' ? "doesn't have" : 'is not';
    case 'lt':
      return kind === 'date' ? 'is before' : 'is less than';
    case 'gt':
      return kind === 'date' ? 'is after' : 'is more than';
    case 'this_week':
      return 'is this week';
    case 'overdue':
      return 'is overdue';
    case 'empty':
      return 'is empty';
    case 'not_empty':
      return 'is not empty';
    case 'checked':
      return 'is checked';
    case 'unchecked':
      return 'is not checked';
  }
}

export const takesValue = (op: FilterOp) =>
  op === 'contains' || op === 'is' || op === 'is_not' || op === 'lt' || op === 'gt';

export function isComplete(rule: FilterRule): boolean {
  if (!takesValue(rule.op)) return true;
  if (rule.op === 'contains') return typeof rule.value === 'string' && rule.value.trim() !== '';
  if (rule.op === 'is' || rule.op === 'is_not') return Array.isArray(rule.value) && rule.value.length > 0;
  return typeof rule.value === 'number' || (typeof rule.value === 'string' && rule.value !== '');
}

export function defaultRule(column: ListColumn): FilterRule {
  const op = OPS_BY_KIND[column.kind][0];
  const firstChoice = column.options.choices?.[0]?.id;
  const value =
    column.kind === 'person'
      ? [ME]
      : (column.kind === 'select' || column.kind === 'multi_select') && firstChoice
        ? [firstChoice]
        : undefined;
  return { column_id: column.id, op, value };
}

export function isoDay(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function weekOf(today: Date): [string, string] {
  const monday = new Date(
    today.getFullYear(),
    today.getMonth(),
    today.getDate() - ((today.getDay() + 6) % 7),
  );
  const sunday = new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + 6);
  return [isoDay(monday), isoDay(sunday)];
}

export interface ViewContext {
  me: string;
  today: Date;
  nameOf: (userId: string) => string;
}

const isEmpty = (value: unknown) =>
  value === undefined || value === null || value === '' || (Array.isArray(value) && value.length === 0);

const idsOf = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((id): id is string => typeof id === 'string') : [];

export function fits(rule: FilterRule, column: ListColumn): boolean {
  if (!OPS_BY_KIND[column.kind].includes(rule.op)) return false;
  return isComplete(rule);
}

export function usableConfig(
  config: ViewConfig,
  kind: ViewKind,
  columns: ListColumn[],
): Required<ViewConfig> {
  const byId = new Map(columns.map((column) => [column.id, column]));
  const group = config.group_by ? byId.get(config.group_by) : undefined;
  return {
    group_by: kind === 'board' && group?.kind === 'select' ? group.id : null,
    sort: (config.sort ?? []).filter((rule) => byId.has(rule.column_id)),
    filters: (config.filters ?? []).filter((rule) => {
      const column = byId.get(rule.column_id);
      return !!column && fits(rule, column);
    }),
  };
}

export function matches(item: ListItem, rule: FilterRule, column: ListColumn, ctx: ViewContext): boolean {
  const value = item.values[column.id];
  switch (rule.op) {
    case 'contains':
      return (
        typeof value === 'string' &&
        typeof rule.value === 'string' &&
        value.toLowerCase().includes(rule.value.toLowerCase())
      );
    case 'is':
    case 'is_not': {
      const wanted = idsOf(rule.value).map((id) => (id === ME ? ctx.me : id));
      const has = column.kind === 'multi_select' ? idsOf(value) : typeof value === 'string' ? [value] : [];
      const hit = has.some((id) => wanted.includes(id));
      return rule.op === 'is' ? hit : !hit;
    }
    case 'lt':
    case 'gt': {
      if (isEmpty(value) || typeof value !== typeof rule.value) return false;
      const [a, b] = [value as number | string, rule.value as number | string];
      return rule.op === 'lt' ? a < b : a > b;
    }
    case 'this_week': {
      if (typeof value !== 'string') return false;
      const [monday, sunday] = weekOf(ctx.today);
      return value >= monday && value <= sunday;
    }
    case 'overdue':
      return typeof value === 'string' && value < isoDay(ctx.today);
    case 'empty':
      return isEmpty(value);
    case 'not_empty':
      return !isEmpty(value);
    case 'checked':
      return value === true;
    case 'unchecked':
      return value !== true;
  }
}

function sortKey(column: ListColumn, value: unknown, ctx: ViewContext): string | number | null {
  if (isEmpty(value)) return null;
  switch (column.kind) {
    case 'number':
      return typeof value === 'number' ? value : null;
    case 'select':
    case 'multi_select': {
      const first = column.kind === 'select' ? value : idsOf(value)[0];
      const choice = choiceOf(column, first);
      return choice ? (column.options.choices ?? []).indexOf(choice) : null;
    }
    case 'person':
      return typeof value === 'string' ? ctx.nameOf(value).toLowerCase() : null;
    case 'checkbox':
      return value === true ? 1 : 0;
    default:
      return typeof value === 'string' ? value.toLowerCase() : null;
  }
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

function compareKeys(a: string | number, b: string | number): number {
  if (typeof a === 'number' && typeof b === 'number') return a - b;
  return collator.compare(String(a), String(b));
}

export function applyView(
  items: ListItem[],
  columns: ListColumn[],
  config: Required<ViewConfig>,
  ctx: ViewContext,
  keep: ReadonlySet<string> = new Set(),
): ListItem[] {
  const byId = new Map(columns.map((column) => [column.id, column]));
  const filters = config.filters.flatMap((rule) => {
    const column = byId.get(rule.column_id);
    return column ? [{ rule, column }] : [];
  });
  const shown =
    filters.length === 0
      ? items
      : items.filter(
          (item) =>
            keep.has(item.id) || filters.every(({ rule, column }) => matches(item, rule, column, ctx)),
        );
  const sorts = config.sort.flatMap((rule) => {
    const column = byId.get(rule.column_id);
    return column ? [{ column, sign: rule.direction === 'desc' ? -1 : 1 }] : [];
  });
  if (sorts.length === 0) return shown;
  const keyed = shown.map((item) => ({
    item,
    keys: sorts.map(({ column }) => sortKey(column, item.values[column.id], ctx)),
  }));
  keyed.sort((a, b) => {
    for (let i = 0; i < sorts.length; i += 1) {
      const [x, y] = [a.keys[i], b.keys[i]];
      if (x === null && y === null) continue;
      if (x === null) return 1;
      if (y === null) return -1;
      const order = compareKeys(x, y) * sorts[i].sign;
      if (order !== 0) return order;
    }
    return byRank(a.item, b.item);
  });
  return keyed.map(({ item }) => item);
}

export function onlyMine(columns: ListColumn[]): FilterRule | null {
  const person = columns.find((column) => column.kind === 'person');
  return person ? { column_id: person.id, op: 'is', value: [ME] } : null;
}

export function dueThisWeek(columns: ListColumn[]): FilterRule | null {
  const date = columns.find((column) => column.kind === 'date');
  return date ? { column_id: date.id, op: 'this_week' } : null;
}

export const sameRule = (a: FilterRule, b: FilterRule) =>
  a.column_id === b.column_id &&
  a.op === b.op &&
  JSON.stringify(a.value ?? null) === JSON.stringify(b.value ?? null);

export function nextViewName(views: SavedView[], kind: ViewKind): string {
  const base = kind === 'board' ? 'Board' : 'Table';
  const taken = new Set(views.map((view) => view.name));
  if (!taken.has(base)) return base;
  let n = 2;
  while (taken.has(`${base} ${n}`)) n += 1;
  return `${base} ${n}`;
}
