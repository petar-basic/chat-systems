import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useWorkspaceStore } from '@/stores/workspace';
import { instanceManager } from '@/lib/instances';
import { api } from '@/lib/api';
import type { Message } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';
import { patchMessageById, claimReplyCount } from '@/lib/messageCache';
import type { MessagesInfiniteData } from './useMessages';

interface ThreadMessagesResponse {
  data: Message[];
}

function useCurrentApi() {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  return instanceUrl ? instanceManager.get(instanceUrl).api : api;
}

export const useThreadMessages = (parentMessageId: string) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.thread(parentMessageId),
    queryFn: async () => {
      const response = await apiClient.get<ThreadMessagesResponse>(`/messages/${parentMessageId}/thread`);
      return response.data;
    },
    staleTime: 1000 * 60,
  });
};

export const useSendThreadReply = (parentMessageId: string, channelId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  return useMutation({
    mutationFn: async (content: string) => {
      return apiClient.post<Message>(`/messages/${parentMessageId}/thread`, {
        content,
        channel_id: channelId,
      });
    },
    onSuccess: (newMessage) => {
      queryClient.setQueryData<Message[]>(QUERY_KEYS.thread(parentMessageId), (old = []) =>
        old.some((m) => m.id === newMessage.id) ? old : [...old, newMessage],
      );
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
        await apiClient.delete(`/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`);
      } else {
        await apiClient.post(`/messages/${messageId}/reactions`, { emoji });
      }
    } finally {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.thread(parentMessageId) });
    }
  };

  const edit = async (messageId: string, content: string) => {
    const updated = await apiClient.patch<Message>(`/messages/${messageId}`, { content });
    patchReply(queryClient, parentMessageId, messageId, (m) => ({ ...m, ...updated }));
  };

  const remove = async (messageId: string) => {
    await apiClient.delete(`/messages/${messageId}`);
    queryClient.setQueryData<Message[]>(QUERY_KEYS.thread(parentMessageId), (old = []) =>
      old.filter((m) => m.id !== messageId),
    );
  };

  const togglePin = async (messageId: string, isPinned: boolean) => {
    patchReply(queryClient, parentMessageId, messageId, (m) => ({ ...m, is_pinned: !isPinned }));
    if (isPinned) await apiClient.delete(`/messages/${messageId}/pin`);
    else await apiClient.post(`/messages/${messageId}/pin`, {});
  };

  return { toggleReaction, edit, remove, togglePin };
};
