import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { Message } from '@/stores/workspace';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { logger } from '@/lib/logger';
import { toast } from '@/shared/components/Toast';
import { QUERY_KEYS, ErrorLabels } from '@/shared/constants';
import { patchMessageById, claimReplyCount } from '@/lib/messageCache';
import type { MessagesInfiniteData } from './useMessages';

export const useThreadMessages = (parentMessageId: string) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.thread(parentMessageId),
    queryFn: async (): Promise<Message[]> => {
      const response = await apiClient.typed((c) =>
        c.GET('/messages/{msg_id}/thread', { params: { path: { msg_id: parentMessageId } } }),
      );
      return response.data;
    },
    staleTime: 1000 * 60,
  });
};

export const useSendThreadReply = (parentMessageId: string, channelId: string, userId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  const key = QUERY_KEYS.thread(parentMessageId);

  return useMutation({
    mutationFn: async ({ content, id }: { content: string; id: string }) =>
      apiClient.typed((c) =>
        c.POST('/channels/{ch_id}/messages', {
          params: { path: { ch_id: channelId } },
          body: { content, client_message_id: id, thread_parent_id: parentMessageId },
        }),
      ),
    onMutate: async ({ content, id }) => {
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
        thread_parent_id: parentMessageId,
        reply_count: 0,
        is_pinned: false,
        pending: true,
      };
      queryClient.setQueryData<Message[]>(key, (old = []) => [...old, optimistic]);
    },
    onError: (err, { id }) => {
      logger.error('useSendThreadReply', 'mutationFn', err);
      queryClient.setQueryData<Message[]>(key, (old = []) =>
        old.map((m) => (m.id === id ? { ...m, pending: false, failed: true } : m)),
      );
      toast.error(ErrorLabels.SendFailed);
    },
    onSuccess: (newMessage, { id }) => {
      queryClient.setQueryData<Message[]>(key, (old = []) => {
        const without = old.filter((m) => m.id !== id && m.id !== newMessage.id);
        return [...without, { ...newMessage, pending: false, failed: false }];
      });
      if (!claimReplyCount(newMessage.id)) return;
      queryClient.setQueryData<MessagesInfiniteData>(QUERY_KEYS.messages(channelId), (old) =>
        patchMessageById(old, parentMessageId, (m) => ({ ...m, reply_count: (m.reply_count || 0) + 1 })),
      );
    },
  });
};

function patchReply(
  queryClient: ReturnType<typeof useQueryClient>,
  parentMessageId: string,
  messageId: string,
  patch: (message: Message) => Message,
) {
  queryClient.setQueryData<Message[]>(QUERY_KEYS.thread(parentMessageId), (old = []) =>
    old.map((m) => (m.id === messageId ? patch(m) : m)),
  );
}

export const useThreadReplyActions = (parentMessageId: string, currentUserId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  const toggleReaction = async (messageId: string, emoji: string, hasOwn: boolean) => {
    patchReply(queryClient, parentMessageId, messageId, (m) => ({
      ...m,
      reactions: hasOwn
        ? (m.reactions ?? []).filter((r) => !(r.user_id === currentUserId && r.emoji === emoji))
        : [
            ...(m.reactions ?? []),
            {
              id: `optimistic-${crypto.randomUUID()}`,
              message_id: messageId,
              user_id: currentUserId,
              emoji,
              created_at: new Date().toISOString(),
            },
          ],
    }));
    try {
      if (hasOwn) {
        await apiClient.typed((c) =>
          c.DELETE('/messages/{msg_id}/reactions/{emoji}', {
            params: { path: { msg_id: messageId, emoji } },
          }),
        );
      } else {
        await apiClient.typed((c) =>
          c.POST('/messages/{msg_id}/reactions', {
            params: { path: { msg_id: messageId } },
            body: { emoji },
          }),
        );
      }
    } finally {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.thread(parentMessageId) });
    }
  };

  const edit = async (messageId: string, content: string) => {
    const updated = await apiClient.typed((c) =>
      c.PATCH('/messages/{msg_id}', { params: { path: { msg_id: messageId } }, body: { content } }),
    );
    patchReply(queryClient, parentMessageId, messageId, (m) => ({ ...m, ...updated }));
  };

  const remove = async (messageId: string) => {
    await apiClient.typed((c) => c.DELETE('/messages/{msg_id}', { params: { path: { msg_id: messageId } } }));
    queryClient.setQueryData<Message[]>(QUERY_KEYS.thread(parentMessageId), (old = []) =>
      old.filter((m) => m.id !== messageId),
    );
  };

  const togglePin = async (messageId: string, isPinned: boolean) => {
    patchReply(queryClient, parentMessageId, messageId, (m) => ({ ...m, is_pinned: !isPinned }));
    const params = { path: { msg_id: messageId } };
    if (isPinned) await apiClient.typed((c) => c.DELETE('/messages/{msg_id}/pin', { params }));
    else await apiClient.typed((c) => c.POST('/messages/{msg_id}/pin', { params }));
  };

  return { toggleReaction, edit, remove, togglePin };
};
