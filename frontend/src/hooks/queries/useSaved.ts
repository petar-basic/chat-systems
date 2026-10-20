import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type SavedItem = components['schemas']['SavedMessageDetail'];

export interface SaveTarget {
  messageId: string;
  note?: string;
}

export const useSavedItems = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.saved(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/saved', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.data;
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: 1000 * 30,
  });
};

export const useSaveMessage = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (target: SaveTarget) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/saved', {
          params: { path: { ws_id: workspaceId } },
          body: { message_id: target.messageId, note: target.note ?? null },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.saved(workspaceId) });
    },
  });
};

export const useUnsaveMessage = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (savedId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/saved/{id}', { params: { path: { id: savedId } } }),
      ),
    onMutate: async (savedId) => {
      await queryClient.cancelQueries({ queryKey: QUERY_KEYS.saved(workspaceId) });
      const previous = queryClient.getQueryData<SavedItem[]>(QUERY_KEYS.saved(workspaceId));
      queryClient.setQueryData<SavedItem[]>(QUERY_KEYS.saved(workspaceId), (old) =>
        old?.filter((item) => item.id !== savedId),
      );
      return { previous };
    },
    onError: (_err, _savedId, ctx) => {
      if (ctx?.previous) queryClient.setQueryData(QUERY_KEYS.saved(workspaceId), ctx.previous);
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.saved(workspaceId) });
    },
  });
};
