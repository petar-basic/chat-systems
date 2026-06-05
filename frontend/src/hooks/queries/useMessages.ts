import { useInfiniteQuery, useMutation, useQueryClient, type InfiniteData } from '@tanstack/react-query';
import type { Message, Reaction } from '@/stores/workspace';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS, MESSAGES_PAGE_SIZE, ErrorLabels } from '@/shared/constants';
import { upsertMessage, patchMessageById, newestFirst } from '@/lib/messageCache';
import { toast } from '@/shared/components/Toast';
import { logger } from '@/lib/logger';

export interface MessagesResponse {
  data: Message[];
}

export type MessagesInfiniteData = InfiniteData<MessagesResponse>;

export const useMessages = (channelId: string | null) => {
  const apiClient = useCurrentApi();

  return useInfiniteQuery({
    queryKey: QUERY_KEYS.messages(channelId ?? ''),
    queryFn: async ({ pageParam }) => {
      if (!channelId) throw new Error('No channel selected');
      const params = new URLSearchParams();
      params.set('limit', String(MESSAGES_PAGE_SIZE));
      if (pageParam) params.set('cursor', pageParam);
      return apiClient.get<MessagesResponse>(`/channels/${channelId}/messages?${params.toString()}`);
    },
    getNextPageParam: (lastPage) => {
      if (lastPage.data.length < MESSAGES_PAGE_SIZE) return undefined;
      return lastPage.data[0]?.id;
    },
    initialPageParam: undefined as string | undefined,
    enabled: !!channelId,
    staleTime: 0,
    select: (data) => ({
      ...data,
      pages: data.pages.map((page) => ({ ...page, data: [...page.data].reverse() })),
    }),
  });
};

export const useSendMessage = (channelId: string, userId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  const key = QUERY_KEYS.messages(channelId);

  return useMutation({
    mutationFn: async ({ content, id }: { content: string; id: string }) => {
      return apiClient.post<Message>(`/channels/${channelId}/messages`, { content, id });
    },
    onMutate: async ({ content, id }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const optimistic: Message = {
        id,
        channel_id: channelId,
        user_id: userId,
        content,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        deleted_at: null,
        thread_parent_id: null,
        reply_count: 0,
        is_pinned: false,
        pending: true,
      };
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        upsertMessage(old, optimistic, 'lastPage', newestFirst),
      );
      return { id };
    },
    onError: (err, { id }) => {
      logger.error('useSendMessage', 'mutationFn', err);
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        patchMessageById(old, id, (m) => ({ ...m, pending: false, failed: true })),
      );
      toast.error(ErrorLabels.SendFailed);
    },
    onSuccess: (_data, { id }) => {
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        patchMessageById(old, id, (m) => ({ ...m, failed: false })),
      );
    },
  });
};

export const useEditMessage = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  return useMutation({
    mutationFn: async ({ messageId, content }: { messageId: string; content: string; channelId: string }) => {
      return apiClient.patch<Message>(`/messages/${messageId}`, { content });
    },
    onMutate: async ({ messageId, content, channelId }) => {
      const key = QUERY_KEYS.messages(channelId);
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<MessagesInfiniteData>(key);
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (m) => ({ ...m, content, updated_at: new Date().toISOString() })),
      );
      return { previous, channelId };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useEditMessage', 'mutationFn', err);
      if (ctx?.previous) queryClient.setQueryData(QUERY_KEYS.messages(ctx.channelId), ctx.previous);
      toast.error(ErrorLabels.EditFailed);
    },
  });
};

export const useDeleteMessage = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  return useMutation({
    mutationFn: async ({ messageId }: { messageId: string; channelId: string }) => {
      await apiClient.delete(`/messages/${messageId}`);
    },
    onSuccess: (_data, { messageId, channelId }) => {
      queryClient.setQueryData<MessagesInfiniteData>(QUERY_KEYS.messages(channelId), (old) =>
        patchMessageById(old, messageId, (m) => ({ ...m, deleted_at: new Date().toISOString() })),
      );
    },
    onError: (err) => {
      logger.error('useDeleteMessage', 'mutationFn', err);
      toast.error(ErrorLabels.DeleteFailed);
    },
  });
};

interface ReactionVars {
  messageId: string;
  channelId: string;
  emoji: string;
  userId: string;
}

export const usePinMessage = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  return useMutation({
    mutationFn: async ({
      messageId,
      isPinned,
    }: {
      messageId: string;
      channelId: string;
      isPinned: boolean;
    }) => {
      if (isPinned) await apiClient.delete(`/messages/${messageId}/pin`);
      else await apiClient.post(`/messages/${messageId}/pin`, {});
    },
    onMutate: async ({ messageId, channelId, isPinned }) => {
      const key = QUERY_KEYS.messages(channelId);
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<MessagesInfiniteData>(key);
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (m) => ({ ...m, is_pinned: !isPinned })),
      );
      return { previous, channelId };
    },
    onError: (err, _vars, ctx) => {
      logger.error('usePinMessage', 'mutationFn', err);
      if (ctx?.previous) queryClient.setQueryData(QUERY_KEYS.messages(ctx.channelId), ctx.previous);
      toast.error(ErrorLabels.PinFailed);
    },
    onSettled: (_data, _err, { channelId }) => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelPins(channelId) });
    },
  });
};

export const useReactToMessage = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  return useMutation({
    mutationFn: async ({ messageId, emoji }: ReactionVars) => {
      return apiClient.post<Reaction>(`/messages/${messageId}/reactions`, { emoji });
    },
    onMutate: async ({ messageId, channelId, emoji, userId }) => {
      const key = QUERY_KEYS.messages(channelId);
      await queryClient.cancelQueries({ queryKey: key });
      const optimistic: Reaction = {
        id: `optimistic-${crypto.randomUUID()}`,
        message_id: messageId,
        user_id: userId,
        emoji,
        created_at: new Date().toISOString(),
      };
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (m) => {
          const reactions = m.reactions ?? [];
          if (reactions.some((r) => r.user_id === userId && r.emoji === emoji)) return m;
          return { ...m, reactions: [...reactions, optimistic] };
        }),
      );
    },
    onError: (err, { messageId, channelId, emoji, userId }) => {
      logger.error('useReactToMessage', 'mutationFn', err);
      queryClient.setQueryData<MessagesInfiniteData>(QUERY_KEYS.messages(channelId), (old) =>
        patchMessageById(old, messageId, (m) => ({
          ...m,
          reactions: (m.reactions ?? []).filter((r) => !(r.user_id === userId && r.emoji === emoji)),
        })),
      );
      toast.error(ErrorLabels.ReactionFailed);
    },
  });
};

export const useRemoveReaction = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  return useMutation({
    mutationFn: async ({ messageId, emoji }: ReactionVars) => {
      return apiClient.delete(`/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`);
    },
    onMutate: async ({ messageId, channelId, emoji, userId }) => {
      const key = QUERY_KEYS.messages(channelId);
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<MessagesInfiniteData>(key);
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (m) => ({
          ...m,
          reactions: (m.reactions ?? []).filter((r) => !(r.user_id === userId && r.emoji === emoji)),
        })),
      );
      return { previous, channelId };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useRemoveReaction', 'mutationFn', err);
      if (ctx?.previous) queryClient.setQueryData(QUERY_KEYS.messages(ctx.channelId), ctx.previous);
      toast.error(ErrorLabels.ReactionFailed);
    },
  });
};
