import { useEffect, useMemo } from 'react';
import { useWorkspaceStore } from '@/stores/workspace';
import { useUserCache } from '@/stores/users';
import { useCustomEmojiStore } from '@/stores/customEmoji';
import { useCustomEmoji } from '@/hooks/queries/useCustomEmoji';
import { useUserGroupStore } from '@/stores/userGroups';
import { useUserGroups } from '@/hooks/queries/useUserGroups';
import { useInstanceStore } from '@/stores/instances';
import { useDocumentTitle } from '@/shared/hooks/useDocumentTitle';
import { useFaviconBadge } from '@/shared/hooks/useFaviconBadge';
import { useWorkspaceUnreadCounts, useMarkChannelNotificationsRead } from '@/hooks/queries/useNotifications';
import {
  useWorkspaces,
  useWorkspaceChannels,
  useWorkspaceMembers,
  useDeletedWorkspaces,
  useRestoreWorkspace,
} from '@/hooks/queries/useWorkspaces';
import { useConversations, useMarkConversationRead } from '@/hooks/queries/useConversations';
import { useUnreadChannels, useSetChannelMuted } from '@/hooks/queries/useChannels';
import { instanceManager } from '@/lib/instances';
import { wsClient } from '@/lib/ws';
import { usePresenceStore } from '@/stores/presence';
import { requestNotificationPermission } from '@/lib/notifications';

/// Everything the workspace shell reads from the server, plus the effects that
/// push it into the stores: unread and muted sets, the user cache, custom
/// emoji, the person's groups, and the channel subscriptions on the socket.
export function useWorkspaceData(workspaceId: string | undefined, currentUserId: string | undefined) {
  const { activeInstanceUrl } = useInstanceStore();
  const {
    currentWorkspace,
    currentConversationId,
    setCurrentUserId,
    hydrateUnreadConversations,
    hydrateUnreadChannels,
    hydrateUnreadCounts,
    hydrateMutedChannels,
  } = useWorkspaceStore();
  const storeWorkspaceId = currentWorkspace?.id;

  const { data: workspaces = [] } = useWorkspaces();
  const { data: deletedWorkspaces = [] } = useDeletedWorkspaces();
  const currentWsInstanceUrl =
    workspaces.find((w) => w.id === workspaceId)?.instanceUrl ?? activeInstanceUrl ?? undefined;
  const { data: channels = [] } = useWorkspaceChannels(workspaceId || null, currentWsInstanceUrl);
  const { data: workspaceMembers = [] } = useWorkspaceMembers(
    workspaceId || storeWorkspaceId || null,
    currentWsInstanceUrl,
  );
  const restoreWorkspace = useRestoreWorkspace();

  const { data: conversations = [] } = useConversations(
    workspaceId || storeWorkspaceId || null,
    currentWsInstanceUrl,
  );
  const { mutate: markConversationReadServer } = useMarkConversationRead(
    workspaceId || storeWorkspaceId || '',
    currentWsInstanceUrl,
  );

  useEffect(() => {
    const unread = conversations
      .filter(
        (c) => c.id !== currentConversationId && (!c.last_read_at || c.last_message_at > c.last_read_at),
      )
      .map((c) => c.id);
    hydrateUnreadConversations(unread);
  }, [conversations, currentConversationId, hydrateUnreadConversations]);

  const { data: unread } = useUnreadChannels(workspaceId || storeWorkspaceId || null, currentWsInstanceUrl);
  useEffect(() => {
    if (!unread) return;
    if (unread.channel_ids.length) hydrateUnreadChannels(unread.channel_ids);
    hydrateUnreadCounts(unread.counts ?? []);
  }, [unread, hydrateUnreadChannels, hydrateUnreadCounts]);

  useEffect(() => {
    hydrateMutedChannels(channels.filter((c) => c.muted).map((c) => c.id));
  }, [channels, hydrateMutedChannels]);

  const { mutate: setChannelMuted } = useSetChannelMuted(
    workspaceId || storeWorkspaceId || '',
    currentWsInstanceUrl,
  );
  const { mutate: markChannelNotificationsRead } = useMarkChannelNotificationsRead(
    workspaceId || storeWorkspaceId || null,
  );

  const unreadByWorkspace = useWorkspaceUnreadCounts(workspaces);
  const totalUnread = useMemo(
    () => Object.values(unreadByWorkspace).reduce((sum, n) => sum + n, 0),
    [unreadByWorkspace],
  );
  useFaviconBadge(totalUnread > 0);
  useDocumentTitle(currentWorkspace ? `Chat Systems - ${currentWorkspace.name}` : 'Chat Systems');

  useEffect(() => {
    if (totalUnread > 0) {
      navigator.setAppBadge?.(totalUnread).catch(() => {});
    } else {
      navigator.clearAppBadge?.().catch(() => {});
    }
  }, [totalUnread]);

  const { populateUsers, getUser } = useUserCache();
  const populateCustomEmoji = useCustomEmojiStore((s) => s.populate);
  const populateSelfGroups = useUserGroupStore((s) => s.populate);
  useEffect(() => {
    if (workspaceMembers.length > 0) {
      populateUsers(
        workspaceMembers.map((m) => ({
          id: m.user_id,
          email: m.email,
          display_name: m.display_name ?? '',
          avatar_url: m.avatar_url ?? null,
          status_emoji: m.status_emoji,
          status_text: m.status_text,
        })),
      );
    }
  }, [workspaceMembers, populateUsers]);

  const { data: customEmoji } = useCustomEmoji(workspaceId, currentWsInstanceUrl);
  useEffect(() => {
    populateCustomEmoji(customEmoji ?? []);
  }, [customEmoji, populateCustomEmoji]);

  const { data: userGroups } = useUserGroups(workspaceId, currentWsInstanceUrl);
  useEffect(() => {
    populateSelfGroups((userGroups ?? []).filter((g) => g.is_member).map((g) => `group:${g.id}`));
  }, [userGroups, populateSelfGroups]);

  useEffect(() => {
    setCurrentUserId(currentUserId ?? null);
  }, [currentUserId, setCurrentUserId]);

  const wsInstanceUrl = currentWorkspace?.instanceUrl;
  useEffect(() => {
    if (!storeWorkspaceId) return;
    const ids = [...channels.map((ch) => ch.id), ...conversations.map((c) => c.id)];
    if (ids.length === 0) return;
    const ws = wsInstanceUrl ? instanceManager.get(wsInstanceUrl).ws : wsClient;
    ws.joinChannels(ids);
  }, [storeWorkspaceId, channels, conversations, wsInstanceUrl]);

  useEffect(() => {
    requestNotificationPermission();
  }, []);

  useEffect(() => {
    const cleanup = usePresenceStore.getState().initPresenceListener();
    return cleanup;
  }, []);

  return {
    workspaces,
    deletedWorkspaces,
    currentWsInstanceUrl,
    channels,
    workspaceMembers,
    conversations,
    restoreWorkspace,
    setChannelMuted,
    markChannelNotificationsRead,
    markConversationReadServer,
    getUser,
  };
}
