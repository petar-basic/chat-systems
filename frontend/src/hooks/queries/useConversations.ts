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

export type ConversationKind = 'direct' | 'group';

export interface Conversation {
  id: string;
  workspace_id: string;
  kind: ConversationKind;
  last_message_at: string;
  last_read_at: string | null;
  participant_ids: string[];
}

export interface ConversationMessage {
  id: string;
  conversation_id: string;
  user_id: string;
  client_message_id?: string | null;
  content: string;
  edited_at: string | null;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
  thread_parent_id: string | null;
  reply_count: number;
  pending?: boolean;
  reactions?: Reaction[];
}

export interface ConversationMessagesPage {
  data: ConversationMessage[];
  next_cursor: string | null;
}

export type ConversationInfiniteData = InfiniteData<ConversationMessagesPage>;

export const useConversations = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.conversations(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: Conversation[] }>(
        `/workspaces/${workspaceId}/conversations`,
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
      getApiForInstance(instanceUrl).post<Conversation>(`/workspaces/${workspaceId}/conversations`, {
        participant_ids: participantIds,
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.conversations(workspaceId) });
    },
  });
};

export const useConversationMessages = (conversationId: string | null, instanceUrl?: string) => {
  return useInfiniteQuery({
    queryKey: QUERY_KEYS.conversationMessages(conversationId ?? ''),
    queryFn: async ({ pageParam }: { pageParam?: string }) => {
      const cursor = pageParam ? `&before=${encodeURIComponent(pageParam)}` : '';
      return getApiForInstance(instanceUrl).get<ConversationMessagesPage>(
        `/conversations/${conversationId}/messages?limit=${MESSAGES_PAGE_SIZE}${cursor}`,
      );
    },
    getNextPageParam: (lastPage) => lastPage.next_cursor ?? undefined,
    initialPageParam: undefined as string | undefined,
    enabled: !!conversationId && !!instanceUrl,
  });
};

export const useSendConversationMessage = (
  workspaceId: string,
  conversationId: string,
  authorId: string,
  instanceUrl?: string,
) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.conversationMessages(conversationId);

  return useMutation({
    mutationFn: async ({ content, id }: { content: string; id: string }) =>
      getApiForInstance(instanceUrl).post<ConversationMessage>(`/conversations/${conversationId}/messages`, {
        content,
        // The server owns the message id; this one only makes a retry of the
        // same send idempotent, and is scoped to this conversation.
        client_message_id: id,
      }),
    onMutate: async ({ content, id }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const optimistic: ConversationMessage = {
        id,
        conversation_id: conversationId,
        user_id: authorId,
        client_message_id: id,
        content,
        edited_at: null,
        deleted_at: null,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        thread_parent_id: null,
        reply_count: 0,
        pending: true,
      };
      queryClient.setQueryData<ConversationInfiniteData>(key, (old) =>
        upsertMessage(old, optimistic, 'firstPage', newestFirst),
      );
      return { id };
    },
    onError: (err, { id }) => {
      logger.error('useSendConversationMessage', 'mutationFn', err);
      queryClient.setQueryData<ConversationInfiniteData>(key, (old) => removeMessageById(old, id));
      toast.error(ErrorLabels.SendFailed);
    },
    onSuccess: (message, { id }) => {
      // The optimistic row was keyed on the client id, the stored row is keyed
      // on the server's. Swapping them here is what stops the websocket echo
      // from arriving as a second copy of the same message.
      queryClient.setQueryData<ConversationInfiniteData>(key, (old) =>
        upsertMessage(removeMessageById(old, id), { ...message, pending: false }, 'firstPage', newestFirst),
      );
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.conversations(workspaceId) });
    },
  });
};

export const useEditConversationMessage = (conversationId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.conversationMessages(conversationId);

  return useMutation({
    mutationFn: async ({ messageId, content }: { messageId: string; content: string }) =>
      getApiForInstance(instanceUrl).patch<ConversationMessage>(`/conversations/messages/${messageId}`, {
        content,
      }),
    onMutate: async ({ messageId, content }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<ConversationInfiniteData>(key);
      queryClient.setQueryData<ConversationInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (m) => ({ ...m, content, edited_at: new Date().toISOString() })),
      );
      return { previous };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useEditConversationMessage', 'mutationFn', err);
      if (ctx?.previous) queryClient.setQueryData(key, ctx.previous);
      toast.error(ErrorLabels.EditFailed);
    },
  });
};

export const useDeleteConversationMessage = (conversationId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.conversationMessages(conversationId);

  return useMutation({
    mutationFn: async ({ messageId }: { messageId: string }) =>
      getApiForInstance(instanceUrl).delete(`/conversations/messages/${messageId}`),
    onMutate: async ({ messageId }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<ConversationInfiniteData>(key);
      queryClient.setQueryData<ConversationInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (m) => ({ ...m, deleted_at: new Date().toISOString() })),
      );
      return { previous };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useDeleteConversationMessage', 'mutationFn', err);
      if (ctx?.previous) queryClient.setQueryData(key, ctx.previous);
      toast.error(ErrorLabels.DeleteFailed);
    },
  });
};

export const useReactToConversationMessage = (
  conversationId: string,
  authorId: string,
  instanceUrl?: string,
) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.conversationMessages(conversationId);

  return useMutation({
    mutationFn: async ({ messageId, emoji }: { messageId: string; emoji: string }) =>
      getApiForInstance(instanceUrl).post<Reaction>(`/conversations/messages/${messageId}/reactions`, {
        emoji,
      }),
    onMutate: async ({ messageId, emoji }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const optimistic: Reaction = {
        id: `optimistic-${crypto.randomUUID()}`,
        message_id: messageId,
        user_id: authorId,
        emoji,
        created_at: new Date().toISOString(),
      };
      queryClient.setQueryData<ConversationInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (msg) => {
          const reactions = msg.reactions ?? [];
          if (reactions.some((r) => r.user_id === authorId && r.emoji === emoji)) return msg;
          return { ...msg, reactions: [...reactions, optimistic] };
        }),
      );
    },
    onError: (err, { messageId, emoji }) => {
      logger.error('useReactToConversationMessage', 'mutationFn', err);
      queryClient.setQueryData<ConversationInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (msg) => ({
          ...msg,
          reactions: (msg.reactions ?? []).filter((r) => !(r.user_id === authorId && r.emoji === emoji)),
        })),
      );
      toast.error(ErrorLabels.ReactionFailed);
    },
  });
};

export const useRemoveConversationReaction = (
  conversationId: string,
  authorId: string,
  instanceUrl?: string,
) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.conversationMessages(conversationId);

  return useMutation({
    mutationFn: async ({ messageId, emoji }: { messageId: string; emoji: string }) =>
      getApiForInstance(instanceUrl).delete(
        `/conversations/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
      ),
    onMutate: async ({ messageId, emoji }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const previous = queryClient.getQueryData<ConversationInfiniteData>(key);
      queryClient.setQueryData<ConversationInfiniteData>(key, (old) =>
        patchMessageById(old, messageId, (msg) => ({
          ...msg,
          reactions: (msg.reactions ?? []).filter((r) => !(r.user_id === authorId && r.emoji === emoji)),
        })),
      );
      return { previous };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useRemoveConversationReaction', 'mutationFn', err);
      if (ctx?.previous) queryClient.setQueryData(key, ctx.previous);
      toast.error(ErrorLabels.ReactionFailed);
    },
  });
};

export const useMarkConversationRead = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (conversationId: string) =>
      getApiForInstance(instanceUrl).post(`/conversations/${conversationId}/read`, {}),
    onMutate: (conversationId) => {
      queryClient.setQueryData<Conversation[]>(QUERY_KEYS.conversations(workspaceId), (old) =>
        old?.map((c) => (c.id === conversationId ? { ...c, last_read_at: new Date().toISOString() } : c)),
      );
    },
    onError: (err) => logger.error('useMarkConversationRead', 'mutationFn', err),
  });
};

export const useConversationThread = (parentMessageId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.conversationThread(parentMessageId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: ConversationMessage[] }>(
        `/conversations/messages/${parentMessageId}/thread`,
      );
      return res.data;
    },
    enabled: !!parentMessageId && !!instanceUrl,
    staleTime: 1000 * 30,
  });
};

export const useSendConversationThreadReply = (
  conversationId: string,
  parentMessageId: string,
  instanceUrl?: string,
) => {
  const queryClient = useQueryClient();
  const threadKey = QUERY_KEYS.conversationThread(parentMessageId);

  return useMutation({
    mutationFn: async (content: string) =>
      getApiForInstance(instanceUrl).post<ConversationMessage>(`/conversations/${conversationId}/messages`, {
        content,
        client_message_id: crypto.randomUUID(),
        thread_parent_id: parentMessageId,
      }),
    onSuccess: (reply) => {
      queryClient.setQueryData<ConversationMessage[]>(threadKey, (old = []) =>
        old.some((m) => m.id === reply.id) ? old : [...old, reply],
      );
      // The count under the parent lives in the feed, which never sees the reply.
      queryClient.setQueryData<ConversationInfiniteData>(
        QUERY_KEYS.conversationMessages(conversationId),
        (old) =>
          patchMessageById(old, parentMessageId, (m) => ({ ...m, reply_count: (m.reply_count ?? 0) + 1 })),
      );
    },
    onError: (err) => {
      logger.error('useSendConversationThreadReply', 'mutationFn', err);
      toast.error(ErrorLabels.SendFailed);
    },
  });
};

export const useConversationThreadReplyActions = (
  parentMessageId: string,
  currentUserId: string,
  instanceUrl?: string,
) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.conversationThread(parentMessageId);

  const patchReply = (messageId: string, patch: (m: ConversationMessage) => ConversationMessage) => {
    queryClient.setQueryData<ConversationMessage[]>(key, (old = []) =>
      old.map((m) => (m.id === messageId ? patch(m) : m)),
    );
  };

  const edit = async (messageId: string, content: string) => {
    patchReply(messageId, (m) => ({ ...m, content, edited_at: new Date().toISOString() }));
    try {
      await getApiForInstance(instanceUrl).patch<ConversationMessage>(
        `/conversations/messages/${messageId}`,
        { content },
      );
    } catch (err) {
      logger.error('useConversationThreadReplyActions', 'edit', err);
      queryClient.invalidateQueries({ queryKey: key });
      toast.error(ErrorLabels.EditFailed);
    }
  };

  const remove = async (messageId: string) => {
    patchReply(messageId, (m) => ({ ...m, deleted_at: new Date().toISOString() }));
    try {
      await getApiForInstance(instanceUrl).delete(`/conversations/messages/${messageId}`);
    } catch (err) {
      logger.error('useConversationThreadReplyActions', 'remove', err);
      queryClient.invalidateQueries({ queryKey: key });
      toast.error(ErrorLabels.DeleteFailed);
    }
  };

  const toggleReaction = async (messageId: string, emoji: string, hasOwn: boolean) => {
    patchReply(messageId, (m) => ({
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
        await getApiForInstance(instanceUrl).delete(
          `/conversations/messages/${messageId}/reactions/${encodeURIComponent(emoji)}`,
        );
      } else {
        await getApiForInstance(instanceUrl).post(`/conversations/messages/${messageId}/reactions`, {
          emoji,
        });
      }
    } catch (err) {
      logger.error('useConversationThreadReplyActions', 'toggleReaction', err);
    } finally {
      queryClient.invalidateQueries({ queryKey: key });
    }
  };

  return { edit, remove, toggleReaction };
};
