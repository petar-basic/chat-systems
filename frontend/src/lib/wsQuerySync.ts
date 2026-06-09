import { useEffect } from 'react';
import { useQueryClient, type InfiniteData, type QueryClient } from '@tanstack/react-query';
import { globalEventBus } from './globalEventBus';
import { upsertMessage, patchMessageById, newestFirst } from './messageCache';
import { showNotification, playNotificationSound } from './notifications';
import type { Message } from '@/stores/workspace';
import type { DmConversation, DmInfiniteData } from '@/hooks/queries/useDm';
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
          const { currentUserRole } = useWorkspaceStore.getState();
          const isWorkspaceAdmin = currentUserRole === 'admin' || currentUserRole === 'owner';
          const instances = useInstanceStore.getState().instances;
          const isInstanceAdmin = instances.some(
            (i) => i.url === currentWorkspace.instanceUrl && i.user.is_instance_admin,
          );
          if (isWorkspaceAdmin || isInstanceAdmin) return;
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

        const { currentChannel, mutedChannels } = useWorkspaceStore.getState();
        if (currentChannel?.id !== message.channel_id && !mutedChannels.has(message.channel_id)) {
          useWorkspaceStore.setState((s) => {
            const nextUnread = new Set(s.unreadChannels);
            nextUnread.add(message.channel_id);
            return { unreadChannels: nextUnread };
          });
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
      globalEventBus.on('dm.new', (event) => {
        const msg = event.message;
        if (!msg?.workspace_id || !msg.from_user_id || !msg.to_user_id) return;

        const { currentUserId, currentDmPartnerId, markDmUnread } = useWorkspaceStore.getState();
        const isIncoming = msg.from_user_id !== currentUserId;
        const partnerId = isIncoming ? msg.from_user_id : msg.to_user_id;

        queryClient.setQueryData<DmInfiniteData>(QUERY_KEYS.dmMessages(msg.workspace_id, partnerId), (old) =>
          upsertMessage(old, { ...msg, pending: false }, 'firstPage', newestFirst),
        );

        queryClient.setQueryData<DmConversation[]>(QUERY_KEYS.dmConversations(msg.workspace_id), (old) => {
          if (!old) return old;
          const previous = old.find((c) => c.partner_id === partnerId);
          const without = old.filter((c) => c.partner_id !== partnerId);
          const lastReadAt = isIncoming ? (previous?.last_read_at ?? null) : msg.created_at;
          return [
            { partner_id: partnerId, last_message_at: msg.created_at, last_read_at: lastReadAt },
            ...without,
          ];
        });

        if (isIncoming && currentDmPartnerId !== partnerId) {
          markDmUnread(partnerId);
          if (!document.hasFocus()) playNotificationSound();
          const sender = useUserCache.getState().getUser(partnerId)?.display_name || 'New message';
          showNotification(sender, msg.content);
        }
      }),

      globalEventBus.on('dm.updated', (event) => {
        const msg = event.message;
        if (!msg?.workspace_id) return;
        const { currentUserId } = useWorkspaceStore.getState();
        const partnerId = msg.from_user_id === currentUserId ? msg.to_user_id : msg.from_user_id;
        queryClient.setQueryData<DmInfiniteData>(QUERY_KEYS.dmMessages(msg.workspace_id, partnerId), (old) =>
          patchMessageById(old, msg.id, (m) => ({ ...m, content: msg.content, edited_at: msg.edited_at })),
        );
      }),

      globalEventBus.on('dm.deleted', (event) => {
        const msg = event.message;
        if (!msg?.workspace_id) return;
        const { currentUserId } = useWorkspaceStore.getState();
        const partnerId = msg.from_user_id === currentUserId ? msg.to_user_id : msg.from_user_id;
        queryClient.setQueryData<DmInfiniteData>(QUERY_KEYS.dmMessages(msg.workspace_id, partnerId), (old) =>
          patchMessageById(old, msg.id, (m) => ({
            ...m,
            deleted_at: msg.deleted_at ?? new Date().toISOString(),
          })),
        );
      }),

      globalEventBus.on('dm.reaction.added', (event) => {
        const { currentUserId } = useWorkspaceStore.getState();
        const partnerId = event.from_user_id === currentUserId ? event.to_user_id : event.from_user_id;
        queryClient.setQueryData<DmInfiniteData>(
          QUERY_KEYS.dmMessages(event.workspace_id, partnerId),
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

      globalEventBus.on('dm.reaction.removed', (event) => {
        const { currentUserId } = useWorkspaceStore.getState();
        const partnerId = event.from_user_id === currentUserId ? event.to_user_id : event.from_user_id;
        queryClient.setQueryData<DmInfiniteData>(
          QUERY_KEYS.dmMessages(event.workspace_id, partnerId),
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
