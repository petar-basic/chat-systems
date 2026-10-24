import type { QueryClient } from '@tanstack/react-query';
import { globalEventBus } from '@/lib/globalEventBus';
import { QUERY_KEYS } from '@/shared/constants';
import { removeItem, upsertItem, type ListDetail, type ListItem } from './listModel';

export function syncListFrames(queryClient: QueryClient): Array<() => void> {
  const setItems = (listId: string, patch: (items: ListItem[]) => ListItem[]) =>
    queryClient.setQueryData<ListItem[]>(QUERY_KEYS.listItems(listId), (items) => items && patch(items));
  const setDetail = (listId: string, patch: (detail: ListDetail) => ListDetail) =>
    queryClient.setQueryData<ListDetail>(QUERY_KEYS.list(listId), (detail) => detail && patch(detail));

  return [
    globalEventBus.on('list.upserted', (event) => {
      setDetail(event.list_id, (detail) => ({ ...detail, list: event.list }));
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.listsOf(event.list.workspace_id) });
    }),
    globalEventBus.on('list.deleted', (event) => {
      queryClient.invalidateQueries({ queryKey: ['lists', 'of'] });
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.list(event.list_id) });
    }),
    globalEventBus.on('list.columns_changed', (event) => {
      setDetail(event.list_id, (detail) => ({ ...detail, columns: event.columns }));
      if (event.items_rewritten) {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.listItems(event.list_id) });
      }
    }),
    globalEventBus.on('list.views_changed', (event) => {
      setDetail(event.list_id, (detail) => ({ ...detail, views: event.views }));
    }),
    globalEventBus.on('list.item_upserted', (event) => {
      setItems(event.list_id, (items) => upsertItem(items, event.item));
    }),
    globalEventBus.on('list.item_deleted', (event) => {
      setItems(event.list_id, (items) => removeItem(items, event.item_id));
    }),
  ];
}
