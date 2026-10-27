import { useCallback, useSyncExternalStore } from 'react';
import { useInfiniteQuery, useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import type { Message } from '@/stores/workspace';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { logger } from '@/lib/logger';
import { refusalOr } from '@/lib/errors';
import { toast } from '@/shared/components/Toast';
import { QUERY_KEYS, ErrorLabels } from '@/shared/constants';
import { patchMessageById, claimReplyCount, newestFirst, upsertMessage } from '@/lib/messageCache';
import { dropReply, oldestFirst, patchReply, placeReply, type ThreadPageOfReplies } from '@/lib/threadCache';
import { shownInChannel } from '@/lib/messagePatch';
import { useUnsentStore } from '@/stores/unsent';
import type { MessagesInfiniteData, OutgoingMessage } from '@/features/messaging';

export type ThreadSummary = components['schemas']['ThreadSummary'];
export type ThreadState = components['schemas']['ThreadState'];
type ThreadPage = components['schemas']['ThreadPage'];

export const useFollowedThreads = (workspaceId: string | undefined) => {
  const apiClient = useCurrentApi();
  return useInfiniteQuery({
    queryKey: QUERY_KEYS.followedThreads(workspaceId ?? ''),
    enabled: !!workspaceId,
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }): Promise<ThreadPage> =>
      apiClient.typed((c) =>
        c.GET('/workspaces/{ws_id}/threads', {
          params: { path: { ws_id: workspaceId ?? '' }, query: { cursor: pageParam } },
        }),
      ),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });
};

export const useUnreadThreadCount = (workspaceId: string | undefined) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.unreadThreads(workspaceId ?? ''),
    enabled: !!workspaceId,
    queryFn: async () => {
      const res = await apiClient.typed((c) =>
        c.GET('/workspaces/{ws_id}/threads/unread', { params: { path: { ws_id: workspaceId ?? '' } } }),
      );
      return res.count;
    },
  });
};

export const useThreadState = (rootId: string) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.threadState(rootId),
    queryFn: (): Promise<ThreadState> =>
      apiClient.typed((c) => c.POST('/threads/{root_id}/read', { params: { path: { root_id: rootId } } })),
    staleTime: Infinity,
  });
};

export const useMarkThreadRead = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: (rootId: string) =>
      apiClient.typed((c) => c.POST('/threads/{root_id}/read', { params: { path: { root_id: rootId } } })),
    onSuccess: (state) => queryClient.setQueryData(QUERY_KEYS.threadState(state.root_message_id), state),
    onError: (err) => logger.error('useMarkThreadRead', 'mutationFn', err),
  });
};

export const useFollowThread = (rootId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: (following: boolean) => {
      const params = { params: { path: { root_id: rootId } } };
      return following
        ? apiClient.typed((c) => c.PUT('/threads/{root_id}/follow', params))
        : apiClient.typed((c) => c.DELETE('/threads/{root_id}/follow', params));
    },
    onSuccess: (state) => queryClient.setQueryData(QUERY_KEYS.threadState(rootId), state),
    onError: (err) => {
      logger.error('useFollowThread', 'mutationFn', err);
      toast.error(ErrorLabels.FollowFailed);
    },
  });
};

const THREAD_PAGE = 50;

export const useThreadMessages = (parentMessageId: string) => {
  const apiClient = useCurrentApi();
  return useInfiniteQuery({
    queryKey: QUERY_KEYS.thread(parentMessageId),
    initialPageParam: null as string | null,
    queryFn: async ({ pageParam }): Promise<ThreadPageOfReplies> => {
      const { data } = await apiClient.typed((c) =>
        c.GET('/messages/{msg_id}/thread', {
          params: { path: { msg_id: parentMessageId }, query: { limit: THREAD_PAGE, cursor: pageParam } },
        }),
      );
      const replies = data as Message[];
      return { data: replies, next: replies.length === THREAD_PAGE ? replies[replies.length - 1].id : null };
    },
    getNextPageParam: (page) => page.next ?? undefined,
    select: oldestFirst,
    staleTime: 1000 * 60,
  });
};

export type ThreadReply = OutgoingMessage & { alsoToChannel?: boolean };

export const useSendThreadReply = (parentMessageId: string, channelId: string, userId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  const key = QUERY_KEYS.thread(parentMessageId);

  return useMutation({
    mutationFn: async ({ content, id, attachments = [], alsoToChannel = false }: ThreadReply) =>
      apiClient.typed((c) =>
        c.POST('/channels/{ch_id}/messages', {
          params: { path: { ch_id: channelId } },
          body: {
            content,
            client_message_id: id,
            thread_parent_id: parentMessageId,
            also_to_channel: alsoToChannel,
            attachment_ids: attachments.map((a) => a.id),
          },
        }),
      ),
    onMutate: async ({ content, id, attachments = [], alsoToChannel = false }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const metadata = {
        ...(attachments.length > 0 ? { attachments } : {}),
        ...(alsoToChannel ? { also_in_channel: true } : {}),
      };
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
        metadata: Object.keys(metadata).length > 0 ? metadata : undefined,
        pending: true,
      };
      useUnsentStore.getState().drop(optimistic);
      placeReply(queryClient, optimistic);
      return { optimistic };
    },
    onError: (err, { id }, context) => {
      logger.error('useSendThreadReply', 'mutationFn', err);
      patchReply(queryClient, id, (m) => ({ ...m, pending: false, failed: true }));
      if (context) useUnsentStore.getState().keep({ ...context.optimistic, pending: false, failed: true });
      toast.error(refusalOr(err, ErrorLabels.SendFailed));
    },
    onSuccess: (newMessage, { id }) => {
      useUnsentStore.getState().drop({ id, channel_id: channelId, thread_parent_id: parentMessageId });
      const sent = { ...(newMessage as Message), pending: false, failed: false };
      placeReply(queryClient, sent, id);
      if (shownInChannel(sent)) {
        queryClient.setQueryData<MessagesInfiniteData>(QUERY_KEYS.messages(channelId), (old) =>
          upsertMessage(old, sent, 'lastPage', newestFirst),
        );
      }
      if (!claimReplyCount(newMessage.id)) return;
      queryClient.setQueryData<MessagesInfiniteData>(QUERY_KEYS.messages(channelId), (old) =>
        patchMessageById(old, parentMessageId, (m) => ({ ...m, reply_count: (m.reply_count || 0) + 1 })),
      );
    },
  });
};

export const useThreadReplyActions = (parentMessageId: string, currentUserId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();

  const toggleReaction = async (messageId: string, emoji: string, hasOwn: boolean) => {
    patchReply(queryClient, messageId, (m) => ({
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
    } catch (err) {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.thread(parentMessageId) });
      throw err;
    }
  };

  const edit = async (messageId: string, content: string, attachmentIds?: string[]) => {
    const updated = await apiClient.typed((c) =>
      c.PATCH('/messages/{msg_id}', {
        params: { path: { msg_id: messageId } },
        body: { content, attachment_ids: attachmentIds ?? null },
      }),
    );
    patchReply(queryClient, messageId, (m) => ({ ...m, ...updated }));
  };

  const remove = async (messageId: string) => {
    await apiClient.typed((c) => c.DELETE('/messages/{msg_id}', { params: { path: { msg_id: messageId } } }));
    dropReply(queryClient, messageId);
  };

  const togglePin = async (messageId: string, isPinned: boolean) => {
    patchReply(queryClient, messageId, (m) => ({ ...m, is_pinned: !isPinned }));
    const params = { path: { msg_id: messageId } };
    if (isPinned) await apiClient.typed((c) => c.DELETE('/messages/{msg_id}/pin', { params }));
    else await apiClient.typed((c) => c.POST('/messages/{msg_id}/pin', { params }));
  };

  return { toggleReaction, edit, remove, togglePin };
};

export const useLiveMessage = (message: Message): Message => {
  const queryClient = useQueryClient();
  const subscribe = useCallback(
    (onChange: () => void) => queryClient.getQueryCache().subscribe(onChange),
    [queryClient],
  );
  const read = useCallback(() => {
    const cache = queryClient.getQueryData<MessagesInfiniteData>(QUERY_KEYS.messages(message.channel_id));
    for (const page of cache?.pages ?? []) {
      const found = page.data.find((m) => m.id === message.id);
      if (found) return found;
    }
    return message;
  }, [queryClient, message]);
  return useSyncExternalStore(subscribe, read);
};
