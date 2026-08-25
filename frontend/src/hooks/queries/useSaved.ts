import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export interface SavedItem {
  id: string;
  workspace_id: string;
  message_id: string | null;
  conversation_message_id: string | null;
  note: string | null;
  created_at: string;
  channel_id: string | null;
  conversation_id: string | null;
  author_id: string;
  content: string;
  sent_at: string;
}

export interface SaveTarget {
  messageId?: string;
  conversationMessageId?: string;
  note?: string;
}

export const useSavedItems = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.saved(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: SavedItem[] }>(
        `/workspaces/${workspaceId}/saved`,
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
      getApiForInstance(instanceUrl).post(`/workspaces/${workspaceId}/saved`, {
        message_id: target.messageId ?? null,
        conversation_message_id: target.conversationMessageId ?? null,
        note: target.note ?? null,
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.saved(workspaceId) });
    },
  });
};

export const useUnsaveMessage = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (savedId: string) => getApiForInstance(instanceUrl).delete(`/saved/${savedId}`),
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
