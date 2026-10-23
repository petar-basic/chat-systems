import { describe, it, expect } from 'vitest';
import {
  cellText,
  movedRank,
  parseDraft,
  rankBetween,
  upsertItem,
  withCells,
  type ListColumn,
  type ListItem,
} from './listModel';

const item = (
  id: string,
  rank: string,
  updated_at = '2026-10-01T00:00:00Z',
  values: ListItem['values'] = {},
) =>
  ({
    id,
    list_id: 'l-1',
    rank,
    values,
    created_by: 'u',
    created_at: updated_at,
    updated_at,
  }) satisfies ListItem;

describe('rankBetween', () => {
  it('matches the server for the cases it is tested on', () => {
    expect(rankBetween(null, null)).toBe('V');
    expect(rankBetween('a', 'b')).toBe('aV');
    expect(rankBetween('az', 'b')).toBe('azV');
  });

  it('keeps finding room between neighbours, at both ends and in one gap', () => {
    let last = rankBetween(null, null);
    let first = last;
    for (let i = 0; i < 300; i += 1) {
      const next = rankBetween(last, null);
      expect(next > last).toBe(true);
      last = next;
      const before = rankBetween(null, first);
      expect(before < first).toBe(true);
      first = before;
    }
    let high = rankBetween('V', null);
    for (let i = 0; i < 100; i += 1) {
      const mid = rankBetween('V', high);
      expect('V' < mid && mid < high).toBe(true);
      expect(mid.endsWith('0')).toBe(false);
      high = mid;
    }
  });

  it('places a moved item between its new neighbours', () => {
    const items = [item('a', 'F'), item('b', 'M'), item('c', 'T')];
    const toTop = movedRank(items, 'c', null);
    expect(toTop < 'F').toBe(true);
    const afterA = movedRank(items, 'c', 'a');
    expect('F' < afterA && afterA < 'M').toBe(true);
  });
});

describe('upsertItem', () => {
  it('keeps rank order and ignores a frame older than what it has', () => {
    const items = [item('a', 'F'), item('c', 'T')];
    const placed = upsertItem(items, item('b', 'M'));
    expect(placed.map((i) => i.id)).toEqual(['a', 'b', 'c']);
    const newer = item('a', 'Z', '2026-10-02T00:00:00Z');
    const moved = upsertItem(placed, newer);
    expect(moved.map((i) => i.id)).toEqual(['b', 'c', 'a']);
    expect(upsertItem(moved, item('a', 'A'))).toBe(moved);
  });

  it('drops a cleared cell', () => {
    const merged = withCells(item('a', 'F', undefined, { x: 1, y: 2 }), { x: null, z: 3 });
    expect(merged.values).toEqual({ y: 2, z: 3 });
  });
});

describe('parseDraft and cellText', () => {
  const choices: ListColumn = {
    id: 'c',
    list_id: 'l',
    position: 0,
    name: 'Tags',
    kind: 'multi_select',
    options: {
      choices: [
        { id: 't-a', label: 'Alpha', color: 'blue' },
        { id: 't-b', label: 'Beta', color: 'red' },
      ],
    },
    hidden: false,
  };

  it('reads each typed kind and clears on blank', () => {
    expect(parseDraft('text', '  ')).toEqual({ value: null });
    expect(parseDraft('number', '2,500')).toEqual({ value: 2500 });
    expect(parseDraft('number', 'x')).toEqual({ error: 'Enter a number' });
    expect(parseDraft('link', 'http://a.dev')).toEqual({ value: 'http://a.dev' });
    expect(parseDraft('link', 'a.dev')).toEqual({ value: 'https://a.dev' });
    expect(parseDraft('date', '24.10.2026')).toEqual({ error: 'Pick a date' });
  });

  it('shows choices by label and forgets ones that no longer exist', () => {
    expect(cellText(choices, ['t-b', 'gone', 't-a'], () => '')).toBe('Beta, Alpha');
  });
});
