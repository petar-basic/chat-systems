import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { globalEventBus } from '@/lib/globalEventBus';
import { QUERY_KEYS } from '@/shared/constants';
import { syncListFrames } from './listSync';
import type { ListColumn, ListDetail, ListItem } from './listModel';

const LIST_ID = 'l-1';

const item = (id: string, rank: string, updated_at: string, values: ListItem['values'] = {}) =>
  ({
    id,
    list_id: LIST_ID,
    rank,
    values,
    created_by: 'u',
    created_at: updated_at,
    updated_at,
  }) satisfies ListItem;

const title: ListColumn = {
  id: 'c-1',
  list_id: LIST_ID,
  position: 0,
  name: 'Title',
  kind: 'text',
  options: {},
  hidden: false,
};

const detail: ListDetail = {
  list: {
    id: LIST_ID,
    workspace_id: 'w-1',
    channel_id: 'ch-1',
    title: 'Launch',
    description: '',
    created_by: 'u',
    created_at: '2026-10-01T00:00:00Z',
    updated_at: '2026-10-01T00:00:00Z',
  },
  columns: [title],
};

let queryClient: QueryClient;
let unsubs: Array<() => void>;

beforeEach(() => {
  queryClient = new QueryClient();
  queryClient.setQueryData(QUERY_KEYS.list(LIST_ID), detail);
  queryClient.setQueryData(QUERY_KEYS.listItems(LIST_ID), [
    item('a', 'F', '2026-10-01T00:00:00Z'),
    item('b', 'M', '2026-10-01T00:00:00Z'),
  ]);
  unsubs = syncListFrames(queryClient);
});

afterEach(() => {
  for (const unsub of unsubs) unsub();
});

const items = () => queryClient.getQueryData<ListItem[]>(QUERY_KEYS.listItems(LIST_ID))!;

describe('syncListFrames', () => {
  it('places an upserted item by rank and removes a deleted one', () => {
    globalEventBus.emit({
      type: 'list.item_upserted',
      list_id: LIST_ID,
      channel_id: 'ch-1',
      item: item('c', 'A', '2026-10-02T00:00:00Z', { 'c-1': 'New' }),
    });
    expect(items().map((i) => i.id)).toEqual(['c', 'a', 'b']);
    globalEventBus.emit({ type: 'list.item_deleted', list_id: LIST_ID, channel_id: 'ch-1', item_id: 'a' });
    expect(items().map((i) => i.id)).toEqual(['c', 'b']);
  });

  it('takes new columns and refetches items only when cells were rewritten', () => {
    const renamed = { ...title, name: 'Task' };
    globalEventBus.emit({
      type: 'list.columns_changed',
      list_id: LIST_ID,
      channel_id: 'ch-1',
      columns: [renamed],
      items_rewritten: false,
    });
    expect(queryClient.getQueryData<ListDetail>(QUERY_KEYS.list(LIST_ID))?.columns).toEqual([renamed]);
    expect(queryClient.getQueryState(QUERY_KEYS.listItems(LIST_ID))?.isInvalidated).toBe(false);
    globalEventBus.emit({
      type: 'list.columns_changed',
      list_id: LIST_ID,
      channel_id: 'ch-1',
      columns: [renamed],
      items_rewritten: true,
    });
    expect(queryClient.getQueryState(QUERY_KEYS.listItems(LIST_ID))?.isInvalidated).toBe(true);
  });

  it('renames the open list', () => {
    globalEventBus.emit({
      type: 'list.upserted',
      list_id: LIST_ID,
      channel_id: 'ch-1',
      list: { ...detail.list, title: 'Launch v2' },
    });
    expect(queryClient.getQueryData<ListDetail>(QUERY_KEYS.list(LIST_ID))?.list.title).toBe('Launch v2');
  });
});
