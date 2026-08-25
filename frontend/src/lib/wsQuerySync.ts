import { useEffect } from 'react';
import { useQueryClient, type InfiniteData, type QueryClient } from '@tanstack/react-query';
import { globalEventBus } from './globalEventBus';
import { logger } from './logger';
import { backfillAfterReconnect } from './realtimeBackfill';
import { upsertMessage, patchMessageById, newestFirst } from './messageCache';
import { showNotification, playNotificationSound } from './notifications';
import type { Message, WorkspaceMember } from '@/stores/workspace';
import type {
  Conversation,
  ConversationInfiniteData,
  ConversationMessage,
} from '@/hooks/queries/useConversations';
import { useWorkspaceStore } from '@/stores/workspace';
import { useInstanceStore } from '@/stores/instances';
import { useUserCache } from '@/stores/users';
import { QUERY_KEYS } from '@/shared/constants';

type ChannelMessages = InfiniteData<{ data: Message[] }>;

function patchChannel(
  queryClient: QueryClient,
  channelId: string,
  updater: (cache: ChannelMessages) => ChannelMessages | undefined,
  invalidateIfAbsent = false,
) {
  const key = QUERY_KEYS.messages(channelId);
  const existing = queryClient.getQueryData<ChannelMessages>(key);
  if (!existing) {
    if (invalidateIfAbsent) queryClient.invalidateQueries({ queryKey: key });
    return;
  }
  queryClient.setQueryData(key, updater(existing));
}

export const useWebSocketQuerySync = () => {
  const queryClient = useQueryClient();

  useEffect(() => {
    const unsubs: Array<() => void> = [];

    unsubs.push(
      globalEventBus.on('workspace.created', () => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
      }),
      globalEventBus.on('workspace.updated', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspace(event.workspace_id) });
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
      }),
      globalEventBus.on('workspace.deleted', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.deletedWorkspaces() });
        const currentWorkspace = useWorkspaceStore.getState().currentWorkspace;
        if (currentWorkspace && currentWorkspace.id === event.workspace_id) {
          const instances = useInstanceStore.getState().instances;
          const instance = instances.find((i) => i.url === currentWorkspace.instanceUrl);
          const members = queryClient.getQueryData<WorkspaceMember[]>(
            QUERY_KEYS.workspaceMembers(currentWorkspace.id),
          );
          const role = members?.find((m) => m.user_id === instance?.user.id)?.role;
          const isWorkspaceAdmin = role === 'admin' || role === 'owner';
          const isInstanceAdmin = instance?.user.is_instance_admin ?? false;
          if (isWorkspaceAdmin || isInstanceAdmin) return;
          useWorkspaceStore.setState({ currentWorkspace: null, currentChannel: null });
          window.history.pushState({}, '', '/app');
        }
      }),
      // The gateway drops the subscription server-side; the client has to stop
      // showing a channel it can no longer read, and forget what it cached.
      globalEventBus.on('channel.access_revoked', (event) => {
        queryClient.removeQueries({ queryKey: QUERY_KEYS.messages(event.channel_id) });
        queryClient.removeQueries({ queryKey: QUERY_KEYS.channelMembers(event.channel_id) });
        if (event.workspace_id) {
          queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(event.workspace_id) });
        }
        const { currentChannel } = useWorkspaceStore.getState();
        if (currentChannel?.id === event.channel_id) {
          useWorkspaceStore.setState({ currentChannel: null });
          window.history.pushState({}, '', '/app');
        }
      }),
      globalEventBus.on('workspace.access_revoked', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
        const { currentWorkspace } = useWorkspaceStore.getState();
        if (currentWorkspace?.id === event.workspace_id) {
          useWorkspaceStore.setState({ currentWorkspace: null, currentChannel: null });
          window.history.pushState({}, '', '/app');
        }
      }),
      globalEventBus.on('workspace.restored', () => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
      }),
    );

    unsubs.push(
      globalEventBus.on('member.added', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(event.workspace_id) });
      }),
      globalEventBus.on('member.removed', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(event.workspace_id) });
      }),
    );

    unsubs.push(
      globalEventBus.on('channel.created', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(event.workspace_id) });
      }),
      globalEventBus.on('channel.updated', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channel(event.channel_id) });
      }),
      globalEventBus.on('channel.member_added', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelMembers(event.channel_id) });
      }),
      globalEventBus.on('channel.member_removed', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelMembers(event.channel_id) });
      }),
    );

    unsubs.push(
      globalEventBus.on('sync.refetch_required', (event) => {
        // The gap was longer than the replay log goes back. Refetching is the
        // honest answer — the alternative is a client quietly missing events it
        // will never be told about.
        logger.info('wsQuerySync', 'sync.refetch_required', event.workspace_id);
        backfillAfterReconnect();
      }),
      globalEventBus.on('sync.complete', (event) => {
        logger.info('wsQuerySync', 'sync.complete', `replayed ${event.replayed} events`);
      }),
    );

    unsubs.push(
      globalEventBus.on('message.new', (event) => {
        const message = event.message;
        if (!message?.channel_id) return;

        if (message.thread_parent_id) {
          queryClient.setQueryData<Message[]>(QUERY_KEYS.thread(message.thread_parent_id), (old = []) =>
            old.some((m) => m.id === message.id) ? old : [...old, message],
          );
          const parentId = message.thread_parent_id;
          patchChannel(
            queryClient,
            message.channel_id,
            (cache) =>
              patchMessageById(cache, parentId, (m) => ({ ...m, reply_count: (m.reply_count || 0) + 1 })),
            true,
          );
          return;
        }

        patchChannel(queryClient, message.channel_id, (cache) =>
          upsertMessage(cache, { ...message, pending: false }, 'lastPage', newestFirst),
        );

        const { currentChannel, mutedChannels, bumpChannelUnread, currentUserId } =
          useWorkspaceStore.getState();
        if (currentChannel?.id !== message.channel_id && !mutedChannels.has(message.channel_id)) {
          useWorkspaceStore.setState((s) => {
            const nextUnread = new Set(s.unreadChannels);
            nextUnread.add(message.channel_id);
            return { unreadChannels: nextUnread };
          });
          // The delta is +1 by construction, so the badge moves without a round
          // trip back to the channel list.
          const mentionedIds = (event.mentioned_user_ids ?? []) as string[];
          bumpChannelUnread(message.channel_id, mentionedIds.includes(currentUserId ?? ''));
        }
      }),

      globalEventBus.on('message.updated', (event) => {
        const message = event.message;
        if (!message?.channel_id) return;
        patchChannel(queryClient, message.channel_id, (cache) =>
          patchMessageById(cache, message.id, (m) => ({
            ...m,
            content: message.content,
            updated_at: message.updated_at,
          })),
        );
      }),

      globalEventBus.on('message.deleted', (event) => {
        patchChannel(
          queryClient,
          event.channel_id,
          (cache) =>
            patchMessageById(cache, event.message_id, (m) => ({
              ...m,
              deleted_at: new Date().toISOString(),
            })),
          true,
        );
      }),

      globalEventBus.on('message.pinned', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelPins(event.channel_id) });
        patchChannel(
          queryClient,
          event.channel_id,
          (cache) => patchMessageById(cache, event.message_id, (m) => ({ ...m, is_pinned: event.pinned })),
          true,
        );
      }),
    );

    unsubs.push(
      globalEventBus.on('reaction.added', (event) => {
        const { channel_id, ...reaction } = event.reaction;
        patchChannel(queryClient, channel_id, (cache) =>
          patchMessageById(cache, event.message_id, (m) => {
            const reactions = m.reactions ?? [];
            if (reactions.some((r) => r.id === reaction.id)) return m;
            return { ...m, reactions: [...reactions, reaction] };
          }),
        );
      }),

      globalEventBus.on('reaction.removed', (event) => {
        patchChannel(queryClient, event.channel_id, (cache) =>
          patchMessageById(cache, event.message_id, (m) => ({
            ...m,
            reactions: (m.reactions ?? []).filter(
              (r) => !(r.user_id === event.user_id && r.emoji === event.emoji),
            ),
          })),
        );
      }),
    );

    unsubs.push(
      globalEventBus.on('conversation.message.created', (event) => {
        const { currentUserId, currentConversationId, markConversationUnread } = useWorkspaceStore.getState();
        const isIncoming = event.user_id !== currentUserId;

        // A threaded reply is not part of the feed — the server keeps it out of
        // the listing too, so putting it in here would be the one place it shows
        // up twice.
        if (event.thread_parent_id) {
          queryClient.setQueryData<ConversationMessage[]>(
            QUERY_KEYS.conversationThread(event.thread_parent_id),
            (old = []) => (old.some((m) => m.id === event.id) ? old : [...old, { ...event }]),
          );
          queryClient.setQueryData<ConversationInfiniteData>(
            QUERY_KEYS.conversationMessages(event.conversation_id),
            (old) =>
              patchMessageById(old, event.thread_parent_id as string, (m) => ({
                ...m,
                reply_count: (m.reply_count ?? 0) + 1,
              })),
          );
        } else {
          queryClient.setQueryData<ConversationInfiniteData>(
            QUERY_KEYS.conversationMessages(event.conversation_id),
            (old) => upsertMessage(old, { ...event, pending: false }, 'firstPage', newestFirst),
          );
        }

        queryClient.setQueryData<Conversation[]>(QUERY_KEYS.conversations(event.workspace_id), (old) => {
          if (!old) return old;
          const previous = old.find((c) => c.id === event.conversation_id);
          if (!previous) return old;
          const without = old.filter((c) => c.id !== event.conversation_id);
          return [
            {
              ...previous,
              last_message_at: event.created_at,
              last_read_at: isIncoming ? previous.last_read_at : event.created_at,
            },
            ...without,
          ];
        });

        if (isIncoming && currentConversationId !== event.conversation_id) {
          markConversationUnread(event.conversation_id);
          if (!document.hasFocus()) playNotificationSound();
          const sender = useUserCache.getState().getUser(event.user_id)?.display_name || 'New message';
          showNotification(sender, event.content);
        }
      }),

      globalEventBus.on('conversation.created', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.conversations(event.workspace_id) });
      }),

      globalEventBus.on('conversation.message.updated', (event) => {
        queryClient.setQueryData<ConversationInfiniteData>(
          QUERY_KEYS.conversationMessages(event.conversation_id),
          (old) =>
            patchMessageById(old, event.id, (m) => ({
              ...m,
              content: event.content,
              edited_at: event.edited_at,
            })),
        );
      }),

      globalEventBus.on('conversation.message.deleted', (event) => {
        queryClient.setQueryData<ConversationInfiniteData>(
          QUERY_KEYS.conversationMessages(event.conversation_id),
          (old) =>
            patchMessageById(old, event.id, (m) => ({
              ...m,
              deleted_at: event.deleted_at ?? new Date().toISOString(),
            })),
        );
      }),

      globalEventBus.on('conversation.reaction.added', (event) => {
        queryClient.setQueryData<ConversationInfiniteData>(
          QUERY_KEYS.conversationMessages(event.conversation_id),
          (old) =>
            patchMessageById(old, event.message_id, (m) => {
              const reactions = m.reactions ?? [];
              if (
                reactions.some(
                  (r) =>
                    r.id === event.reaction.id ||
                    (r.user_id === event.reaction.user_id && r.emoji === event.reaction.emoji),
                )
              ) {
                return m;
              }
              return { ...m, reactions: [...reactions, event.reaction] };
            }),
        );
      }),

      globalEventBus.on('conversation.reaction.removed', (event) => {
        queryClient.setQueryData<ConversationInfiniteData>(
          QUERY_KEYS.conversationMessages(event.conversation_id),
          (old) =>
            patchMessageById(old, event.message_id, (m) => ({
              ...m,
              reactions: (m.reactions ?? []).filter(
                (r) => !(r.user_id === event.user_id && r.emoji === event.emoji),
              ),
            })),
        );
      }),
    );

    unsubs.push(
      globalEventBus.on('huddle.started', (event) => {
        if (!event.channel_id) return;
        useWorkspaceStore.getState().setChannelHuddle(event.channel_id, {
          huddleId: event.huddle_id,
          initiatorId: event.initiator_id,
        });
      }),
      globalEventBus.on('huddle.ended', (event) => {
        if (!event.channel_id) return;
        useWorkspaceStore.getState().clearChannelHuddle(event.channel_id);
      }),
    );

    return () => {
      for (const unsub of unsubs) unsub();
    };
  }, [queryClient]);
};
