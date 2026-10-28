import { useInfiniteQuery, useMutation, useQueryClient, type InfiniteData } from '@tanstack/react-query';
import type { Message, Reaction } from '@/stores/workspace';
import type { components } from '@/api/schema';

type Attachment = components['schemas']['Attachment'];
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { applyReadState } from '@/lib/readState';
import { settleChannelNotifications } from '@/features/notifications';
import { QUERY_KEYS, MESSAGES_PAGE_SIZE, ErrorLabels } from '@/shared/constants';
import { upsertMessage, patchMessageById, newestFirst, removeMessageById } from '@/lib/messageCache';
import { toast } from '@/shared/components/Toast';
import { logger } from '@/lib/logger';
import { refusalOr } from '@/lib/errors';
import { useUnsentStore } from '@/stores/unsent';

export interface OutgoingMessage {
  content: string;
  id: string;
  attachments?: Attachment[];
}

export interface MessagesResponse {
  data: Message[];
  unread_notifications?: number | null;
  last_read_before?: string | null;
}

export type MessagesInfiniteData = InfiniteData<MessagesResponse>;

export function messagesOldestFirst(data: MessagesInfiniteData | undefined): Message[] {
  if (!data) return [];
  return [...data.pages].reverse().flatMap((page) => page.data);
}

export function olderMessagesCursor(page: MessagesResponse): string | undefined {
  if (page.data.length < MESSAGES_PAGE_SIZE) return undefined;
  const oldest = page.data[page.data.length - 1];
  return oldest?.id;
}

export const useMessages = (channelId: string | null, markReadIn?: string) => {
  const apiClient = useCurrentApi();
  const queryClient = useQueryClient();

  return useInfiniteQuery({
    queryKey: QUERY_KEYS.messages(channelId ?? ''),
    queryFn: async ({ pageParam }): Promise<MessagesResponse> => {
      if (!channelId) throw new Error('No channel selected');
      const page = await apiClient.typed((c) =>
        c.GET('/channels/{ch_id}/messages', {
          params: {
            path: { ch_id: channelId },
            query: {
              limit: MESSAGES_PAGE_SIZE,
              cursor: pageParam,
              mark_read: markReadIn && pageParam === undefined ? true : undefined,
            },
          },
        }),
      );
      if (markReadIn && page.unread_notifications != null) {
        await settleChannelNotifications(queryClient, markReadIn, channelId, page.unread_notifications);
      }
      return page;
    },
    getNextPageParam: olderMessagesCursor,
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
    mutationFn: async ({ content, id, attachments = [] }: OutgoingMessage) => {
      // The server owns the message id; this one only makes a retry of the same
      // send idempotent, and is scoped to this channel.
      return apiClient.typed((c) =>
        c.POST('/channels/{ch_id}/messages', {
          params: { path: { ch_id: channelId } },
          body: { content, client_message_id: id, attachment_ids: attachments.map((a) => a.id) },
        }),
      );
    },
    onMutate: async ({ content, id, attachments = [] }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const optimistic: Message = {
        id,
        channel_id: channelId,
        user_id: userId,
        client_message_id: id,
        content,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        deleted_at: null,
        thread_parent_id: null,
        reply_count: 0,
        is_pinned: false,
        metadata: attachments.length > 0 ? { attachments } : undefined,
        pending: true,
      };
      useUnsentStore.getState().drop(optimistic);
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        upsertMessage(old, optimistic, 'lastPage', newestFirst),
      );
      return { optimistic };
    },
    onError: (err, { id }, context) => {
      logger.error('useSendMessage', 'mutationFn', err);
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        patchMessageById(old, id, (m) => ({ ...m, pending: false, failed: true })),
      );
      if (context) useUnsentStore.getState().keep({ ...context.optimistic, pending: false, failed: true });
      toast.error(refusalOr(err, ErrorLabels.SendFailed));
    },
    onSuccess: (message, { id }) => {
      useUnsentStore.getState().deliver({ id, channel_id: channelId, thread_parent_id: null });
      // The optimistic row was keyed on the client id, the stored row is keyed
      // on the server's. Swapping them here is what stops the websocket echo
      // from arriving as a second copy of the same message.
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        upsertMessage(
          removeMessageById(old, id),
          { ...message, pending: false, failed: false },
          'lastPage',
          newestFirst,
        ),
      );
    },
  });
};

export const useEditMessage = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  return useMutation({
    mutationFn: async ({
      messageId,
      content,
      attachmentIds,
    }: {
      messageId: string;
      content: string;
      channelId: string;
      attachmentIds?: string[];
    }) => {
      return apiClient.typed((c) =>
        c.PATCH('/messages/{msg_id}', {
          params: { path: { msg_id: messageId } },
          body: { content, attachment_ids: attachmentIds ?? null },
        }),
      );
    },
    onMutate: async ({ messageId, content, channelId, attachmentIds }) => {
      const key = QUERY_KEYS.messages(channelId);
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<MessagesInfiniteData>(key);
      queryClient.setQueryData<MessagesInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (m) => ({
          ...m,
          content,
          updated_at: new Date().toISOString(),
          metadata:
            attachmentIds && m.metadata
              ? {
                  ...m.metadata,
                  attachments: (m.metadata.attachments ?? []).filter((a) => attachmentIds.includes(a.id)),
                }
              : m.metadata,
        })),
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
      await apiClient.typed((c) =>
        c.DELETE('/messages/{msg_id}', { params: { path: { msg_id: messageId } } }),
      );
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

export const useRefreshLinkPreviews = () => {
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: ({ messageId }: { messageId: string }) =>
      apiClient.typed((c) =>
        c.POST('/messages/{msg_id}/unfurls/refresh', { params: { path: { msg_id: messageId } } }),
      ),
    onError: (err) => logger.error('useRefreshLinkPreviews', 'mutationFn', err),
  });
};

export const useHideLinkPreviews = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  return useMutation({
    mutationFn: async ({ messageId }: { messageId: string; channelId: string }) =>
      apiClient.typed((c) =>
        c.DELETE('/messages/{msg_id}/unfurls', { params: { path: { msg_id: messageId } } }),
      ),
    onSuccess: (message, { messageId, channelId }) => {
      queryClient.setQueryData<MessagesInfiniteData>(QUERY_KEYS.messages(channelId), (old) =>
        patchMessageById(old, messageId, (m) => ({ ...m, metadata: message.metadata })),
      );
    },
    onError: (err) => {
      logger.error('useHideLinkPreviews', 'mutationFn', err);
      toast.error(ErrorLabels.HidePreviewFailed);
    },
  });
};

export const useMarkUnread = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  return useMutation({
    mutationFn: async ({ messageId, channelId }: { messageId: string; channelId: string }) =>
      apiClient.typed((c) =>
        c.POST('/channels/{ch_id}/unread', {
          params: { path: { ch_id: channelId } },
          body: { message_id: messageId },
        }),
      ),
    onSuccess: (state) => applyReadState(queryClient, state),
    onError: (err) => {
      logger.error('useMarkUnread', 'mutationFn', err);
      toast.error(ErrorLabels.MarkUnreadFailed);
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
      const params = { path: { msg_id: messageId } };
      if (isPinned) await apiClient.typed((c) => c.DELETE('/messages/{msg_id}/pin', { params }));
      else await apiClient.typed((c) => c.POST('/messages/{msg_id}/pin', { params }));
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
      return apiClient.typed((c) =>
        c.POST('/messages/{msg_id}/reactions', { params: { path: { msg_id: messageId } }, body: { emoji } }),
      );
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
      return apiClient.typed((c) =>
        c.DELETE('/messages/{msg_id}/reactions/{emoji}', { params: { path: { msg_id: messageId, emoji } } }),
      );
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
