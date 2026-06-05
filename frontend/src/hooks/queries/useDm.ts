import {
  useQuery,
  useInfiniteQuery,
  useMutation,
  useQueryClient,
  type InfiniteData,
} from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS, MESSAGES_PAGE_SIZE, ErrorLabels } from '@/shared/constants';
import { upsertMessage, removeMessageById, patchMessageById, newestFirst } from '@/lib/messageCache';
import { toast } from '@/shared/components/Toast';
import { logger } from '@/lib/logger';
import type { Reaction } from '@/stores/workspace';

export interface DirectMessage {
  id: string;
  workspace_id: string;
  from_user_id: string;
  to_user_id: string;
  content: string;
  edited_at: string | null;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
  pending?: boolean;
  reactions?: Reaction[];
}

export interface DmConversation {
  partner_id: string;
  last_message_at: string;
  last_read_at: string | null;
}

export interface DmMessagesPage {
  data: DirectMessage[];
  next_cursor: string | null;
}

export type DmInfiniteData = InfiniteData<DmMessagesPage>;

export const useDmConversations = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.dmConversations(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: DmConversation[] }>(
        `/workspaces/${workspaceId}/dm`,
      );
      return [...res.data].sort((a, b) => b.last_message_at.localeCompare(a.last_message_at));
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: 1000 * 60,
  });
};

export const useDirectMessages = (
  workspaceId: string | null,
  partnerId: string | null,
  instanceUrl?: string,
) => {
  return useInfiniteQuery({
    queryKey: QUERY_KEYS.dmMessages(workspaceId ?? '', partnerId ?? ''),
    queryFn: async ({ pageParam }: { pageParam?: string }) => {
      const cursor = pageParam ? `&before=${encodeURIComponent(pageParam)}` : '';
      return getApiForInstance(instanceUrl).get<DmMessagesPage>(
        `/workspaces/${workspaceId}/dm/${partnerId}?limit=${MESSAGES_PAGE_SIZE}${cursor}`,
      );
    },
    getNextPageParam: (lastPage) => lastPage.next_cursor ?? undefined,
    initialPageParam: undefined as string | undefined,
    enabled: !!workspaceId && !!partnerId && !!instanceUrl,
  });
};

export const useSendDirectMessage = (
  workspaceId: string,
  partnerId: string,
  fromUserId: string,
  instanceUrl?: string,
) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.dmMessages(workspaceId, partnerId);

  return useMutation({
    mutationFn: async ({ content, id }: { content: string; id: string }) => {
      return getApiForInstance(instanceUrl).post<DirectMessage>(
        `/workspaces/${workspaceId}/dm/${partnerId}`,
        { content, id },
      );
    },
    onMutate: async ({ content, id }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const optimistic: DirectMessage = {
        id,
        workspace_id: workspaceId,
        from_user_id: fromUserId,
        to_user_id: partnerId,
        content,
        edited_at: null,
        deleted_at: null,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        pending: true,
      };
      queryClient.setQueryData<DmInfiniteData>(key, (old) =>
        upsertMessage(old, optimistic, 'firstPage', newestFirst),
      );
      return { id };
    },
    onError: (err, { id }) => {
      logger.error('useSendDirectMessage', 'mutationFn', err);
      queryClient.setQueryData<DmInfiniteData>(key, (old) => removeMessageById(old, id));
      toast.error(ErrorLabels.SendFailed);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.dmConversations(workspaceId) });
    },
  });
};

export const useReactToDm = (
  workspaceId: string,
  partnerId: string,
  fromUserId: string,
  instanceUrl?: string,
) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.dmMessages(workspaceId, partnerId);
  return useMutation({
    mutationFn: async ({ messageId, emoji }: { messageId: string; emoji: string }) =>
      getApiForInstance(instanceUrl).post<Reaction>(
        `/workspaces/${workspaceId}/dm/${partnerId}/${messageId}/reactions`,
        { emoji },
      ),
    onMutate: async ({ messageId, emoji }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const optimistic: Reaction = {
        id: `optimistic-${crypto.randomUUID()}`,
        message_id: messageId,
        user_id: fromUserId,
        emoji,
        created_at: new Date().toISOString(),
      };
      queryClient.setQueryData<DmInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (msg) => {
          const reactions = msg.reactions ?? [];
          if (reactions.some((r) => r.user_id === fromUserId && r.emoji === emoji)) return msg;
          return { ...msg, reactions: [...reactions, optimistic] };
        }),
      );
    },
    onError: (err, { messageId, emoji }) => {
      logger.error('useReactToDm', 'mutationFn', err);
      queryClient.setQueryData<DmInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (msg) => ({
          ...msg,
          reactions: (msg.reactions ?? []).filter((r) => !(r.user_id === fromUserId && r.emoji === emoji)),
        })),
      );
      toast.error(ErrorLabels.ReactionFailed);
    },
  });
};

export const useRemoveDmReaction = (
  workspaceId: string,
  partnerId: string,
  fromUserId: string,
  instanceUrl?: string,
) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.dmMessages(workspaceId, partnerId);
  return useMutation({
    mutationFn: async ({ messageId, emoji }: { messageId: string; emoji: string }) =>
      getApiForInstance(instanceUrl).delete(
        `/workspaces/${workspaceId}/dm/${partnerId}/${messageId}/reactions/${encodeURIComponent(emoji)}`,
      ),
    onMutate: async ({ messageId, emoji }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<DmInfiniteData>(key);
      queryClient.setQueryData<DmInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (msg) => ({
          ...msg,
          reactions: (msg.reactions ?? []).filter((r) => !(r.user_id === fromUserId && r.emoji === emoji)),
        })),
      );
      return { previous };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useRemoveDmReaction', 'mutationFn', err);
      if (ctx?.previous) queryClient.setQueryData(key, ctx.previous);
      toast.error(ErrorLabels.ReactionFailed);
    },
  });
};

export const useMarkDmRead = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (partnerId: string) =>
      getApiForInstance(instanceUrl).post(`/workspaces/${workspaceId}/dm/${partnerId}/read`, {}),
    onMutate: (partnerId) => {
      queryClient.setQueryData<DmConversation[]>(QUERY_KEYS.dmConversations(workspaceId), (old) =>
        old?.map((c) => (c.partner_id === partnerId ? { ...c, last_read_at: new Date().toISOString() } : c)),
      );
    },
    onError: (err) => logger.error('useMarkDmRead', 'mutationFn', err),
  });
};

export const useEditDirectMessage = (workspaceId: string, partnerId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.dmMessages(workspaceId, partnerId);

  return useMutation({
    mutationFn: async ({ messageId, content }: { messageId: string; content: string }) => {
      return getApiForInstance(instanceUrl).patch<DirectMessage>(
        `/workspaces/${workspaceId}/dm/${partnerId}/${messageId}`,
        { content },
      );
    },
    onMutate: async ({ messageId, content }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<DmInfiniteData>(key);
      queryClient.setQueryData<DmInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (m) => ({ ...m, content, edited_at: new Date().toISOString() })),
      );
      return { previous };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useEditDirectMessage', 'mutationFn', err);
      if (ctx?.previous) queryClient.setQueryData(key, ctx.previous);
      toast.error(ErrorLabels.EditFailed);
    },
  });
};

export const useDeleteDirectMessage = (workspaceId: string, partnerId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.dmMessages(workspaceId, partnerId);

  return useMutation({
    mutationFn: async ({ messageId }: { messageId: string }) => {
      return getApiForInstance(instanceUrl).delete(`/workspaces/${workspaceId}/dm/${partnerId}/${messageId}`);
    },
    onMutate: async ({ messageId }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<DmInfiniteData>(key);
      queryClient.setQueryData<DmInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (m) => ({ ...m, deleted_at: new Date().toISOString() })),
      );
      return { previous };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useDeleteDirectMessage', 'mutationFn', err);
      if (ctx?.previous) queryClient.setQueryData(key, ctx.previous);
      toast.error(ErrorLabels.DeleteFailed);
    },
  });
};
