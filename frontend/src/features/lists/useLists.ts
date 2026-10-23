import { useMutation, useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { ErrorLabels, QUERY_KEYS } from '@/shared/constants';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import {
  byRank,
  movedRank,
  removeItem,
  upsertItem,
  withCells,
  withoutColumn,
  type CellValues,
  type ListColumn,
  type ListDetail,
  type ListItem,
} from './listModel';

export type CreateListRequest = components['schemas']['CreateListRequest'];
export type NewColumn = components['schemas']['NewColumn'];
export type UpdateColumnRequest = components['schemas']['UpdateColumnRequest'];
export type ColumnChangePreview = components['schemas']['ColumnChangePreview'];

interface Snapshot {
  before: ListItem[] | undefined;
}

const api = (instanceUrl?: string) => getApiForInstance(instanceUrl);

function patchDetail(queryClient: QueryClient, listId: string, patch: (detail: ListDetail) => ListDetail) {
  queryClient.setQueryData<ListDetail>(QUERY_KEYS.list(listId), (detail) => detail && patch(detail));
}

function patchItems(queryClient: QueryClient, listId: string, patch: (items: ListItem[]) => ListItem[]) {
  queryClient.setQueryData<ListItem[]>(QUERY_KEYS.listItems(listId), (items) => items && patch(items));
}

export function useChannelLists(
  workspaceId: string,
  channelId: string,
  instanceUrl?: string,
  enabled = true,
) {
  return useQuery({
    queryKey: QUERY_KEYS.channelLists(workspaceId, channelId),
    queryFn: async () => {
      const res = await api(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/lists', {
          params: { path: { ws_id: workspaceId }, query: { channel_id: channelId } },
        }),
      );
      return res.data;
    },
    enabled,
  });
}

export function useListDetail(listId: string, instanceUrl?: string) {
  return useQuery({
    queryKey: QUERY_KEYS.list(listId),
    queryFn: () =>
      api(instanceUrl).typed((c) => c.GET('/lists/{list_id}', { params: { path: { list_id: listId } } })),
    retry: false,
  });
}

export function useListItems(listId: string, instanceUrl?: string) {
  return useQuery({
    queryKey: QUERY_KEYS.listItems(listId),
    queryFn: async () => {
      const res = await api(instanceUrl).typed((c) =>
        c.GET('/lists/{list_id}/items', { params: { path: { list_id: listId } } }),
      );
      return [...res.data].sort(byRank);
    },
    retry: false,
  });
}

export function useCreateList(workspaceId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateListRequest) =>
      api(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/lists', { params: { path: { ws_id: workspaceId } }, body }),
      ),
    onSuccess: (detail) => {
      queryClient.setQueryData(QUERY_KEYS.list(detail.list.id), detail);
      queryClient.setQueryData<ListItem[]>(QUERY_KEYS.listItems(detail.list.id), []);
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.listsOf(workspaceId) });
    },
  });
}

export function useRenameList(listId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (title: string) =>
      api(instanceUrl).typed((c) =>
        c.PATCH('/lists/{list_id}', { params: { path: { list_id: listId } }, body: { title } }),
      ),
    onSuccess: (list) => {
      patchDetail(queryClient, listId, (detail) => ({ ...detail, list }));
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.listsOf(list.workspace_id) });
    },
    onError: (err) => toast.error(toUserMessage(err, ErrorLabels.ListFailed)),
  });
}

export function useDeleteList(listId: string, workspaceId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () =>
      api(instanceUrl).typed((c) => c.DELETE('/lists/{list_id}', { params: { path: { list_id: listId } } })),
    onSuccess: () => {
      queryClient.removeQueries({ queryKey: QUERY_KEYS.list(listId) });
      queryClient.removeQueries({ queryKey: QUERY_KEYS.listItems(listId) });
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.listsOf(workspaceId) });
    },
  });
}

export function useColumnActions(listId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  const replaceColumn = (column: ListColumn) =>
    patchDetail(queryClient, listId, (detail) => ({
      ...detail,
      columns: detail.columns.map((c) => (c.id === column.id ? column : c)),
    }));
  const failed = (err: unknown) => toast.error(toUserMessage(err, ErrorLabels.ListColumnFailed));

  const add = useMutation({
    mutationFn: (body: NewColumn) =>
      api(instanceUrl).typed((c) =>
        c.POST('/lists/{list_id}/columns', { params: { path: { list_id: listId } }, body }),
      ),
    onSuccess: (column) =>
      patchDetail(queryClient, listId, (detail) => ({
        ...detail,
        columns: detail.columns.some((c) => c.id === column.id)
          ? detail.columns
          : [...detail.columns, column],
      })),
  });

  const update = useMutation({
    mutationFn: ({ columnId, body }: { columnId: string; body: UpdateColumnRequest }) =>
      api(instanceUrl).typed((c) =>
        c.PATCH('/list-columns/{column_id}', { params: { path: { column_id: columnId } }, body }),
      ),
    onSuccess: (column, { body }) => {
      replaceColumn(column);
      if (body.kind || body.options) {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.listItems(listId) });
      }
    },
  });

  const setHidden = useMutation({
    mutationFn: ({ columnId, hidden }: { columnId: string; hidden: boolean }) =>
      api(instanceUrl).typed((c) =>
        c.PATCH('/list-columns/{column_id}', { params: { path: { column_id: columnId } }, body: { hidden } }),
      ),
    onMutate: ({ columnId, hidden }) =>
      patchDetail(queryClient, listId, (detail) => ({
        ...detail,
        columns: detail.columns.map((c) => (c.id === columnId ? { ...c, hidden } : c)),
      })),
    onSuccess: replaceColumn,
    onError: (err) => {
      failed(err);
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.list(listId) });
    },
  });

  const preview = (columnId: string, body: ColumnChangePreview) =>
    api(instanceUrl).typed((c) =>
      c.POST('/list-columns/{column_id}/preview', { params: { path: { column_id: columnId } }, body }),
    );

  const remove = useMutation({
    mutationFn: (columnId: string) =>
      api(instanceUrl).typed((c) =>
        c.DELETE('/list-columns/{column_id}', { params: { path: { column_id: columnId } } }),
      ),
    onSuccess: (_result, columnId) => {
      patchDetail(queryClient, listId, (detail) => ({
        ...detail,
        columns: detail.columns.filter((c) => c.id !== columnId),
      }));
      patchItems(queryClient, listId, (items) => withoutColumn(items, columnId));
    },
    onError: failed,
  });

  const reorder = useMutation({
    mutationFn: (columnIds: string[]) =>
      api(instanceUrl).typed((c) =>
        c.PUT('/lists/{list_id}/column-order', {
          params: { path: { list_id: listId } },
          body: { column_ids: columnIds },
        }),
      ),
    onMutate: (columnIds) =>
      patchDetail(queryClient, listId, (detail) => ({
        ...detail,
        columns: columnIds
          .map((id, position) => {
            const column = detail.columns.find((c) => c.id === id);
            return column && { ...column, position };
          })
          .filter((c): c is ListColumn => !!c),
      })),
    onSuccess: (res) => patchDetail(queryClient, listId, (detail) => ({ ...detail, columns: res.data })),
    onError: (err) => {
      failed(err);
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.list(listId) });
    },
  });

  return { add, update, setHidden, preview, remove, reorder };
}

export function useItemActions(listId: string, instanceUrl?: string) {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.listItems(listId);
  const snapshot = async (): Promise<Snapshot> => {
    await queryClient.cancelQueries({ queryKey: key });
    return { before: queryClient.getQueryData<ListItem[]>(key) };
  };
  const rollback = (label: string) => (err: unknown, _vars: unknown, context: Snapshot | undefined) => {
    if (context?.before) queryClient.setQueryData(key, context.before);
    toast.error(toUserMessage(err, label));
  };
  const place = (item: ListItem) => patchItems(queryClient, listId, (items) => upsertItem(items, item));

  const create = useMutation({
    mutationFn: (body: { values?: CellValues; after_id?: string | null }) =>
      api(instanceUrl).typed((c) =>
        c.POST('/lists/{list_id}/items', { params: { path: { list_id: listId } }, body }),
      ),
    onSuccess: place,
    onError: (err) => toast.error(toUserMessage(err, ErrorLabels.ListItemFailed)),
  });

  const update = useMutation({
    mutationFn: ({ itemId, values }: { itemId: string; values: CellValues }) =>
      api(instanceUrl).typed((c) =>
        c.PATCH('/list-items/{item_id}', { params: { path: { item_id: itemId } }, body: { values } }),
      ),
    onMutate: async ({ itemId, values }) => {
      const context = await snapshot();
      patchItems(queryClient, listId, (items) =>
        items.map((item) => (item.id === itemId ? withCells(item, values) : item)),
      );
      return context;
    },
    onSuccess: place,
    onError: rollback(ErrorLabels.ListCellFailed),
  });

  const remove = useMutation({
    mutationFn: (itemId: string) =>
      api(instanceUrl).typed((c) =>
        c.DELETE('/list-items/{item_id}', { params: { path: { item_id: itemId } } }),
      ),
    onMutate: async (itemId) => {
      const context = await snapshot();
      patchItems(queryClient, listId, (items) => removeItem(items, itemId));
      return context;
    },
    onError: rollback(ErrorLabels.ListItemFailed),
  });

  const move = useMutation({
    mutationFn: ({ itemId, afterId }: { itemId: string; afterId: string | null }) =>
      api(instanceUrl).typed((c) =>
        c.POST('/list-items/{item_id}/move', {
          params: { path: { item_id: itemId } },
          body: { after_id: afterId },
        }),
      ),
    onMutate: async ({ itemId, afterId }) => {
      const context = await snapshot();
      patchItems(queryClient, listId, (items) => {
        const item = items.find((i) => i.id === itemId);
        if (!item) return items;
        return upsertItem(removeItem(items, itemId), { ...item, rank: movedRank(items, itemId, afterId) });
      });
      return context;
    },
    onSuccess: place,
    onError: rollback(ErrorLabels.ListItemFailed),
  });

  return { create, update, remove, move };
}
