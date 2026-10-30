import { useEffect } from 'react';
import { useQueryClient, type InfiniteData, type QueryClient } from '@tanstack/react-query';
import { globalEventBus } from './globalEventBus';
import { logger } from './logger';
import { refetchAfterGap } from './realtimeBackfill';
import {
  upsertMessage,
  patchMessageById,
  newestFirst,
  claimReplyCount,
  removeMessageById,
  deleteMessageById,
} from './messageCache';
import type { Message, WorkspaceMember } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging/hooks/useConversations';
import { instanceManager } from './instances';
import { wsClient } from './ws';
import { useWorkspaceStore } from '@/stores/workspace';
import { useInstanceStore } from '@/stores/instances';
import { useUserCache } from '@/stores/users';
import { useUnsentStore } from '@/stores/unsent';
import { QUERY_KEYS } from '@/shared/constants';
import { applyReadState } from './readState';
import { applyChannelArchived } from './channelArchive';
import { belongsElsewhere } from './channelWorkspace';
import { readOnScreen } from './readOnScreen';
import { patchMessageEverywhere, shownInChannel } from './messagePatch';
import { dropReply, patchReply, placeReply } from './threadCache';
import { syncListFrames } from '@/features/lists/listSync';
import { applyCommandResponse } from './commandAnswers';

type ChannelMessages = InfiniteData<{ data: Message[] }>;

const pendingUserLookups = new Set<string>();
let userLookupTimer: ReturnType<typeof setTimeout> | null = null;

function ensureUserKnown(queryClient: QueryClient, userId: string | null | undefined) {
  if (!userId) return;
  if (useUserCache.getState().users.has(userId)) return;
  pendingUserLookups.add(userId);
  if (userLookupTimer) return;
  userLookupTimer = setTimeout(() => {
    userLookupTimer = null;
    pendingUserLookups.clear();
    const workspaceId = useWorkspaceStore.getState().currentWorkspace?.id;
    if (!workspaceId) return;
    queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });
  }, 300);
}

function patchChannel(
  queryClient: QueryClient,
  channelId: string,
  updater: (cache: ChannelMessages) => ChannelMessages | undefined,
  invalidateIfAbsent = false,
) {
  const key = QUERY_KEYS.messages(channelId);
  const existing = queryClient.getQueryData<ChannelMessages>(key);
  if (!existing) {
    if (invalidateIfAbsent) {
      void queryClient
        .cancelQueries({ queryKey: key })
        .then(() => queryClient.invalidateQueries({ queryKey: key }));
    }
    return;
  }
  queryClient.setQueryData(key, updater(existing));
}

export const useWebSocketQuerySync = () => {
  const queryClient = useQueryClient();

  useEffect(() => {
    const unsubs: Array<() => void> = [];

    unsubs.push(
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
      globalEventBus.on('channel.read_state', (event) => {
        const here =
          !event.workspace_id || event.workspace_id === useWorkspaceStore.getState().currentWorkspace?.id;
        if (here) applyReadState(queryClient, event);
        if (!event.workspace_id) return;
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.unreads(event.workspace_id) });
        if (event.unread_count === 0)
          queryClient.invalidateQueries({ queryKey: QUERY_KEYS.notifications(event.workspace_id) });
      }),
      globalEventBus.on('command.response', applyCommandResponse),
      globalEventBus.on('later.updated', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.later(event.workspace_id) });
      }),
      globalEventBus.on('thread.read_state', (event) => {
        queryClient.setQueryData(QUERY_KEYS.threadState(event.root_message_id), {
          root_message_id: event.root_message_id,
          following: event.following,
          unread_count: event.unread_count,
        });
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.followedThreads(event.workspace_id) });
        if (event.unread_count === 0)
          queryClient.invalidateQueries({ queryKey: QUERY_KEYS.notifications(event.workspace_id) });
      }),
      globalEventBus.on('channel.updated', (event) => {
        applyChannelArchived(queryClient, event.channel_id, event.workspace_id, event.is_archived);
      }),
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
      globalEventBus.on('typing.indicator', (event) => {
        ensureUserKnown(queryClient, event.user_id);
      }),
    );

    unsubs.push(...syncListFrames(queryClient));

    unsubs.push(
      globalEventBus.on('sync.refetch_required', (event) => {
        // The gap was longer than the replay log goes back. Refetching is the
        // honest answer — the alternative is a client quietly missing events it
        // will never be told about.
        logger.info('wsQuerySync', 'sync.refetch_required', event.workspace_id);
        refetchAfterGap();
      }),
      globalEventBus.on('sync.complete', (event) => {
        logger.info('wsQuerySync', 'sync.complete', `replayed ${event.replayed} events`);
      }),
    );

    unsubs.push(
      globalEventBus.on('message.new', (event) => {
        const message = event.message;
        if (!message?.channel_id) return;
        ensureUserKnown(queryClient, message.user_id);
        if (message.client_message_id) {
          useUnsentStore.getState().deliver({ ...message, id: message.client_message_id });
        }

        if (message.thread_parent_id) {
          const parentId = message.thread_parent_id;
          placeReply(queryClient, message, message.client_message_id ?? undefined);
          if (claimReplyCount(message.id)) {
            patchChannel(
              queryClient,
              message.channel_id,
              (cache) =>
                patchMessageById(cache, parentId, (m) => ({ ...m, reply_count: (m.reply_count || 0) + 1 })),
              true,
            );
          }
          if (!shownInChannel(message)) return;
        }

        patchChannel(
          queryClient,
          message.channel_id,
          (cache) =>
            upsertMessage(
              message.client_message_id ? removeMessageById(cache, message.client_message_id) : cache,
              { ...message, pending: false },
              'lastPage',
              newestFirst,
            ),
          true,
        );

        const {
          currentWorkspace,
          currentChannel,
          currentConversationId,
          mutedChannels,
          bumpChannelUnread,
          markConversationUnread,
          currentUserId,
        } = useWorkspaceStore.getState();
        const isIncoming = message.user_id !== currentUserId;
        if (belongsElsewhere(queryClient, message.channel_id)) return;
        if (isIncoming) readOnScreen(queryClient, message.channel_id);

        const conversationsKey = QUERY_KEYS.conversations(currentWorkspace?.id ?? '');
        const conversations = queryClient.getQueryData<Conversation[]>(conversationsKey);
        const conversation = conversations?.find((c) => c.id === message.channel_id);
        if (conversation) {
          queryClient.setQueryData<Conversation[]>(conversationsKey, (old = []) => [
            {
              ...conversation,
              last_message_at: message.created_at,
              last_read_at: isIncoming ? conversation.last_read_at : message.created_at,
            },
            ...old.filter((c) => c.id !== conversation.id),
          ]);
          if (isIncoming && currentConversationId !== message.channel_id) {
            markConversationUnread(message.channel_id);
          }
          return;
        }

        if (
          isIncoming &&
          currentChannel?.id !== message.channel_id &&
          !mutedChannels.has(message.channel_id)
        ) {
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

      globalEventBus.on('message.new', () => {
        const workspaceId = useWorkspaceStore.getState().currentWorkspace?.id;
        if (workspaceId) queryClient.invalidateQueries({ queryKey: QUERY_KEYS.unreads(workspaceId) });
      }),
      globalEventBus.on('message.updated', (event) => {
        const message = event.message;
        if (!message?.channel_id) return;
        patchMessageEverywhere(queryClient, message, (m) => ({
          ...m,
          content: message.content,
          metadata: message.metadata,
          updated_at: message.updated_at,
          reply_count: message.reply_count,
          deleted_at: message.deleted_at,
        }));
      }),

      globalEventBus.on('message.deleted', (event) => {
        dropReply(queryClient, event.message_id);
        patchChannel(
          queryClient,
          event.channel_id,
          (cache) => deleteMessageById(cache, event.message_id),
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
        patchReply(queryClient, event.message_id, (m) => ({ ...m, is_pinned: event.pinned }));
      }),
    );

    unsubs.push(
      globalEventBus.on('reaction.added', (event) => {
        const { channel_id, ...reaction } = event.reaction;
        ensureUserKnown(queryClient, reaction.user_id);
        const add = (m: Message): Message => {
          const reactions = m.reactions ?? [];
          const known = reactions.some(
            (r) => r.id === reaction.id || (r.user_id === reaction.user_id && r.emoji === reaction.emoji),
          );
          return known ? m : { ...m, reactions: [...reactions, reaction] };
        };
        patchChannel(queryClient, channel_id, (cache) => patchMessageById(cache, event.message_id, add));
        patchReply(queryClient, event.message_id, add);
      }),

      globalEventBus.on('reaction.removed', (event) => {
        const remove = (m: Message): Message => ({
          ...m,
          reactions: (m.reactions ?? []).filter(
            (r) => !(r.user_id === event.user_id && r.emoji === event.emoji),
          ),
        });
        patchChannel(queryClient, event.channel_id, (cache) =>
          patchMessageById(cache, event.message_id, remove),
        );
        patchReply(queryClient, event.message_id, remove);
      }),
    );

    unsubs.push(
      globalEventBus.on('conversation.created', (event) => {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.conversations(event.workspace_id) });
        const instanceUrl = useWorkspaceStore.getState().currentWorkspace?.instanceUrl;
        const ws = instanceUrl ? instanceManager.get(instanceUrl).ws : wsClient;
        ws.joinChannels([event.conversation_id]);
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
