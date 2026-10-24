import { describe, it, expect } from 'vitest';
import type { ListColumn, ListItem } from './listModel';
import {
  applyView,
  dueThisWeek,
  isComplete,
  matches,
  nextViewName,
  onlyMine,
  usableConfig,
  weekOf,
  type SavedView,
  type ViewContext,
} from './viewModel';

const column = (id: string, name: string, kind: ListColumn['kind'], options: ListColumn['options'] = {}) =>
  ({ id, list_id: 'l-1', position: 0, name, kind, options, hidden: false, due: false }) satisfies ListColumn;

const TITLE = column('c-title', 'Task', 'text');
const STATUS = column('c-status', 'Status', 'select', {
  choices: [
    { id: 's-todo', label: 'To do', color: 'gray' },
    { id: 's-doing', label: 'Doing', color: 'blue' },
    { id: 's-done', label: 'Done', color: 'green' },
  ],
});
const OWNER = column('c-owner', 'Owner', 'person');
const DUE = column('c-due', 'Due', 'date');
const POINTS = column('c-points', 'Points', 'number');
const COLUMNS = [TITLE, STATUS, OWNER, DUE, POINTS];

const item = (id: string, rank: string, values: ListItem['values']): ListItem => ({
  id,
  list_id: 'l-1',
  rank,
  values,
  comment_count: 0,
  created_by: 'u-ana',
  created_at: '2026-10-01T00:00:00Z',
  updated_at: '2026-10-01T00:00:00Z',
});

const ITEMS = [
  item('a', 'B', {
    'c-title': 'Fix login',
    'c-status': 's-done',
    'c-owner': 'u-ana',
    'c-due': '2026-10-20',
    'c-points': 3,
  }),
  item('b', 'C', {
    'c-title': 'Write docs',
    'c-status': 's-todo',
    'c-owner': 'u-bob',
    'c-due': '2026-10-28',
  }),
  item('c', 'D', { 'c-title': 'ship it', 'c-status': 's-doing', 'c-points': 8 }),
  item('d', 'E', { 'c-title': 'Login copy', 'c-owner': 'u-ana', 'c-due': '2026-11-02', 'c-points': 1 }),
];

const CTX: ViewContext = {
  me: 'u-ana',
  today: new Date(2026, 9, 28),
  nameOf: (id) => ({ 'u-ana': 'Ana', 'u-bob': 'Bob' })[id] ?? 'Someone',
};

const ids = (items: ListItem[]) => items.map((i) => i.id);

describe('applyView', () => {
  it('keeps rank order with no rules', () => {
    expect(ids(applyView(ITEMS, COLUMNS, usableConfig({}, 'table', COLUMNS), CTX))).toEqual([
      'a',
      'b',
      'c',
      'd',
    ]);
  });

  it('sorts by choice order, then puts empty cells last whichever the direction', () => {
    const asc = usableConfig({ sort: [{ column_id: 'c-status' }] }, 'table', COLUMNS);
    expect(ids(applyView(ITEMS, COLUMNS, asc, CTX))).toEqual(['b', 'c', 'a', 'd']);
    const desc = usableConfig({ sort: [{ column_id: 'c-status', direction: 'desc' }] }, 'table', COLUMNS);
    expect(ids(applyView(ITEMS, COLUMNS, desc, CTX))).toEqual(['a', 'c', 'b', 'd']);
  });

  it('breaks ties with the next sort and then with rank', () => {
    const config = usableConfig(
      { sort: [{ column_id: 'c-owner' }, { column_id: 'c-points', direction: 'desc' }] },
      'table',
      COLUMNS,
    );
    expect(ids(applyView(ITEMS, COLUMNS, config, CTX))).toEqual(['a', 'd', 'b', 'c']);
  });

  it('sorts text the way people read it', () => {
    const config = usableConfig({ sort: [{ column_id: 'c-title' }] }, 'table', COLUMNS);
    expect(ids(applyView(ITEMS, COLUMNS, config, CTX))).toEqual(['a', 'd', 'c', 'b']);
  });

  it('ands its filters together and keeps rows made here visible', () => {
    const config = usableConfig(
      {
        filters: [
          { column_id: 'c-owner', op: 'is', value: ['me'] },
          { column_id: 'c-title', op: 'contains', value: 'LOGIN' },
          { column_id: 'c-points', op: 'gt', value: 2 },
        ],
      },
      'table',
      COLUMNS,
    );
    expect(ids(applyView(ITEMS, COLUMNS, config, CTX))).toEqual(['a']);
    expect(ids(applyView(ITEMS, COLUMNS, config, CTX, new Set(['c'])))).toEqual(['a', 'c']);
  });

  it('ignores rules on columns that are gone or no longer fit', () => {
    const config = usableConfig(
      {
        group_by: 'c-title',
        sort: [{ column_id: 'gone' }],
        filters: [
          { column_id: 'gone', op: 'empty' },
          { column_id: 'c-title', op: 'overdue' },
          { column_id: 'c-title', op: 'contains', value: '  ' },
        ],
      },
      'board',
      COLUMNS,
    );
    expect(config).toEqual({ group_by: null, sort: [], filters: [] });
  });
});

describe('date filters', () => {
  it('reads this week as Monday to Sunday around today', () => {
    expect(weekOf(new Date(2026, 9, 28))).toEqual(['2026-10-26', '2026-11-01']);
    expect(weekOf(new Date(2026, 10, 1))).toEqual(['2026-10-26', '2026-11-01']);
  });

  it('finds what is due this week and what is overdue', () => {
    const week = dueThisWeek(COLUMNS)!;
    expect(ITEMS.filter((i) => matches(i, week, DUE, CTX)).map((i) => i.id)).toEqual(['b']);
    const overdue = { column_id: 'c-due', op: 'overdue' as const };
    expect(ITEMS.filter((i) => matches(i, overdue, DUE, CTX)).map((i) => i.id)).toEqual(['a']);
  });
});

describe('presets and drafts', () => {
  it('builds Only mine from the first person column', () => {
    expect(onlyMine(COLUMNS)).toEqual({ column_id: 'c-owner', op: 'is', value: ['me'] });
    expect(onlyMine([TITLE])).toBeNull();
  });

  it('treats a rule with nothing to compare against as a draft', () => {
    expect(isComplete({ column_id: 'c-title', op: 'contains', value: '' })).toBe(false);
    expect(isComplete({ column_id: 'c-points', op: 'lt', value: 0 })).toBe(true);
    expect(isComplete({ column_id: 'c-due', op: 'this_week' })).toBe(true);
  });

  it('names a new view after the ones already there', () => {
    const view = (name: string) => ({ name }) as SavedView;
    expect(nextViewName([view('Table')], 'board')).toBe('Board');
    expect(nextViewName([view('Table'), view('Table 2')], 'table')).toBe('Table 3');
  });
});
