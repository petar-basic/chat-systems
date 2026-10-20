import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { logger } from '@/lib/logger';
import { QUERY_KEYS } from '@/shared/constants';

export type Conversation = components['schemas']['ConversationSummary'];

export const useConversations = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.conversations(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/conversations', { params: { path: { ws_id: workspaceId } } }),
      );
      return [...res.data].sort((a, b) => b.last_message_at.localeCompare(a.last_message_at));
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: 1000 * 60,
  });
};

export const useOpenConversation = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (participantIds: string[]) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/conversations', {
          params: { path: { ws_id: workspaceId } },
          body: { participant_ids: participantIds },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.conversations(workspaceId) });
    },
  });
};

export const useMarkConversationRead = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (conversationId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/conversations/{conv_id}/read', { params: { path: { conv_id: conversationId } } }),
      ),
    onMutate: (conversationId) => {
      queryClient.setQueryData<Conversation[]>(QUERY_KEYS.conversations(workspaceId), (old) =>
        old?.map((c) => (c.id === conversationId ? { ...c, last_read_at: new Date().toISOString() } : c)),
      );
    },
    onError: (err) => logger.error('useMarkConversationRead', 'mutationFn', err),
  });
};
