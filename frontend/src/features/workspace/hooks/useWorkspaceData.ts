import { useEffect, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useWorkspaceStore, type FormerMember, type WorkspaceMember } from '@/stores/workspace';
import { useUserCache } from '@/stores/users';
import { useCustomEmojiStore } from '@/stores/customEmoji';
import { useCustomEmoji } from '@/features/workspace/hooks/useCustomEmoji';
import { useUserGroupStore } from '@/stores/userGroups';
import { useUserGroups } from '@/features/workspace/hooks/useUserGroups';
import { useKeywordStore } from '@/stores/keywords';
import { useNotificationKeywords, useWorkspaceUnreadCounts } from '@/features/notifications';
import { useInstanceStore } from '@/stores/instances';
import { useDocumentTitle } from '@/shared/hooks/useDocumentTitle';
import { useFaviconBadge } from '@/shared/hooks/useFaviconBadge';
import {
  useWorkspaces,
  useWorkspaceChannels,
  useWorkspaceMembers,
  useFormerMembers,
  useDeletedWorkspaces,
  useRestoreWorkspace,
} from '@/features/workspace/hooks/useWorkspaces';
import { useConversations, useMarkConversationRead } from '@/features/messaging';
import { useUnreadChannels } from '@/features/channel';
import { instanceManager } from '@/lib/instances';
import { wsClient } from '@/lib/ws';
import { usePresenceStore } from '@/stores/presence';
import { requestNotificationPermission } from '@/lib/notifications';
import { knownPeople } from '@/lib/userHelpers';

const NO_MEMBERS: WorkspaceMember[] = [];
const NO_FORMER_MEMBERS: FormerMember[] = [];

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
  } = useWorkspaceStore(
    useShallow((s) => ({
      currentWorkspace: s.currentWorkspace,
      currentConversationId: s.currentConversationId,
      setCurrentUserId: s.setCurrentUserId,
      hydrateUnreadConversations: s.hydrateUnreadConversations,
      hydrateUnreadChannels: s.hydrateUnreadChannels,
      hydrateUnreadCounts: s.hydrateUnreadCounts,
      hydrateMutedChannels: s.hydrateMutedChannels,
    })),
  );
  const storeWorkspaceId = currentWorkspace?.id;

  const { data: workspaces = [], isSuccess: workspacesLoaded } = useWorkspaces();
  const { data: deletedWorkspaces = [], isSuccess: deletedLoaded } = useDeletedWorkspaces();
  const noWorkspaces =
    workspacesLoaded && deletedLoaded && workspaces.length === 0 && deletedWorkspaces.length === 0;
  const currentWsInstanceUrl =
    workspaces.find((w) => w.id === workspaceId)?.instanceUrl ?? activeInstanceUrl ?? undefined;
  const { data: channels = [], isPending: channelsPending } = useWorkspaceChannels(
    workspaceId || null,
    currentWsInstanceUrl,
  );
  const { data: workspaceMembers = NO_MEMBERS } = useWorkspaceMembers(
    workspaceId || storeWorkspaceId || null,
    currentWsInstanceUrl,
  );
  const { data: formerMembers = NO_FORMER_MEMBERS } = useFormerMembers(
    workspaceId || storeWorkspaceId || null,
    currentWsInstanceUrl,
  );
  const restoreWorkspace = useRestoreWorkspace();

  const {
    data: conversations = [],
    isSuccess: conversationsLoaded,
    refetch: refetchConversations,
  } = useConversations(workspaceId || storeWorkspaceId || null, currentWsInstanceUrl);
  const { mutate: markConversationReadServer } = useMarkConversationRead(
    workspaceId || storeWorkspaceId || '',
    currentWsInstanceUrl,
  );

  useEffect(() => {
    const unread = conversations
      .filter((c) => c.id !== currentConversationId && c.unread_count > 0)
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
    if (workspaceMembers.length > 0 || formerMembers.length > 0) {
      populateUsers(knownPeople(workspaceMembers, formerMembers));
    }
  }, [workspaceMembers, formerMembers, populateUsers]);

  const { data: customEmoji } = useCustomEmoji(workspaceId, currentWsInstanceUrl);
  useEffect(() => {
    populateCustomEmoji(customEmoji ?? []);
  }, [customEmoji, populateCustomEmoji]);

  const { data: userGroups } = useUserGroups(workspaceId, currentWsInstanceUrl);
  useEffect(() => {
    populateSelfGroups((userGroups ?? []).filter((g) => g.is_member).map((g) => `group:${g.id}`));
  }, [userGroups, populateSelfGroups]);

  const populateKeywords = useKeywordStore((s) => s.populate);
  const { data: keywords } = useNotificationKeywords(workspaceId || null, currentWsInstanceUrl);
  useEffect(() => {
    populateKeywords(keywords ?? []);
  }, [keywords, populateKeywords]);

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
    noWorkspaces,
    currentWsInstanceUrl,
    channels,
    channelsPending,
    workspaceMembers,
    conversations,
    conversationsLoaded,
    refetchConversations,
    restoreWorkspace,
    markConversationReadServer,
    getUser,
  };
}
