import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { useQueryClient, type InfiniteData } from '@tanstack/react-query';
import { useCurrentUser, useLogout } from '@/hooks/queries/useAuth';
import { useWorkspaceStore, type Message, type Channel, type WorkspaceRole } from '@/stores/workspace';
import { useUserCache } from '@/stores/users';
import { instanceManager } from '@/lib/instances';
import { api } from '@/lib/api';
import { wsClient } from '@/lib/ws';
import { usePresenceStore } from '@/stores/presence';
import { requestNotificationPermission } from '@/lib/notifications';
import { logger } from '@/lib/logger';
import { toast } from '@/shared/components/Toast';
import { ErrorLabels, ROUTES, QUERY_KEYS } from '@/shared/constants';
import { useDocumentTitle } from '@/shared/hooks/useDocumentTitle';
import { useFaviconBadge } from '@/shared/hooks/useFaviconBadge';
import { useWorkspaceUnreadCounts, useMarkChannelNotificationsRead } from '@/hooks/queries/useNotifications';
import {
  useWorkspaces,
  useWorkspaceChannels,
  useWorkspaceMembers,
  useDeletedWorkspaces,
  useRestoreWorkspace,
  useCreateWorkspace,
  useCreateChannel,
} from '@/hooks/queries/useWorkspaces';
import { useDmConversations, useMarkDmRead } from '@/hooks/queries/useDm';
import { useUnreadChannelIds, useSetChannelMuted } from '@/hooks/queries/useChannels';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useSendMessage } from '@/hooks/queries/useMessages';
import { useInstanceStore } from '@/stores/instances';
import { useRightPanel } from './useRightPanel';

interface MessagesResponse {
  data: Message[];
}

export function useWorkspaceController() {
  const { data: user } = useCurrentUser();
  const logout = useLogout();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const {
    workspaceId,
    channelId: urlChannelId,
    messageId: urlMessageId,
    dmUserId,
  } = useParams<{
    workspaceId?: string;
    channelId?: string;
    messageId?: string;
    dmUserId?: string;
  }>();
  const [searchParams] = useSearchParams();
  const { activeInstanceUrl } = useInstanceStore();

  const { data: workspaces = [] } = useWorkspaces();
  const { data: deletedWorkspaces = [] } = useDeletedWorkspaces();
  const currentWsInstanceUrl =
    workspaces.find((w) => w.id === workspaceId)?.instanceUrl ?? activeInstanceUrl ?? undefined;
  const { data: channels = [] } = useWorkspaceChannels(workspaceId || null, currentWsInstanceUrl);
  const { data: workspaceMembers = [] } = useWorkspaceMembers(workspaceId || null, currentWsInstanceUrl);

  const createWorkspaceMutation = useCreateWorkspace();
  const createChannelMutation = useCreateChannel();
  const restoreWorkspace = useRestoreWorkspace();

  const {
    currentWorkspace,
    currentChannel,
    unreadChannels,
    mentionChannels,
    mutedChannels,
    currentDmPartnerId,
    unreadDmPartners,
    selectWorkspace,
    selectChannel,
    selectDmPartner,
    setCurrentUserRole,
    setCurrentUserId,
    markChannelRead,
    markDmRead,
    hydrateUnreadDms,
    hydrateUnreadChannels,
    hydrateMutedChannels,
  } = useWorkspaceStore();

  const currentWorkspaceId = currentWorkspace?.id;

  const { data: dmConversations = [] } = useDmConversations(
    workspaceId || currentWorkspace?.id || null,
    currentWsInstanceUrl,
  );
  const { mutate: markDmReadServer } = useMarkDmRead(
    workspaceId || currentWorkspace?.id || '',
    currentWsInstanceUrl,
  );

  useEffect(() => {
    const unread = dmConversations
      .filter(
        (c) => c.partner_id !== currentDmPartnerId && (!c.last_read_at || c.last_message_at > c.last_read_at),
      )
      .map((c) => c.partner_id);
    hydrateUnreadDms(unread);
  }, [dmConversations, currentDmPartnerId, hydrateUnreadDms]);

  const { data: unreadChannelIds } = useUnreadChannelIds(
    workspaceId || currentWorkspace?.id || null,
    currentWsInstanceUrl,
  );
  useEffect(() => {
    if (unreadChannelIds && unreadChannelIds.length) hydrateUnreadChannels(unreadChannelIds);
  }, [unreadChannelIds, hydrateUnreadChannels]);

  useEffect(() => {
    hydrateMutedChannels(channels.filter((c) => c.muted).map((c) => c.id));
  }, [channels, hydrateMutedChannels]);

  const { mutate: setChannelMuted } = useSetChannelMuted(
    workspaceId || currentWorkspace?.id || '',
    currentWsInstanceUrl,
  );

  const { mutate: markChannelNotificationsRead } = useMarkChannelNotificationsRead(
    workspaceId || currentWorkspace?.id || null,
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

  const { populateUsers } = useUserCache();
  useEffect(() => {
    if (workspaceMembers.length > 0) {
      populateUsers(
        workspaceMembers.map((m) => ({
          id: m.user_id,
          email: m.email,
          display_name: m.display_name ?? '',
          avatar_url: m.avatar_url,
        })),
      );
    }
  }, [workspaceMembers, populateUsers]);

  useEffect(() => {
    if (!user || workspaceMembers.length === 0) return;
    const mine = workspaceMembers.find((m) => m.user_id === user.id);
    setCurrentUserRole(mine ? (mine.role as WorkspaceRole) : null);
  }, [workspaceMembers, user, setCurrentUserRole]);

  useEffect(() => {
    setCurrentUserId(user?.id ?? null);
  }, [user?.id, setCurrentUserId]);

  const [uploading, setUploading] = useState(false);
  const [showProfile, setShowProfile] = useState(false);
  const [showAddInstance, setShowAddInstance] = useState(false);
  const [quickSwitcherOpen, setQuickSwitcherOpen] = useState(false);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setQuickSwitcherOpen((v) => !v);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const panel = useRightPanel(currentChannel?.id, currentDmPartnerId);

  const sendMessageMutation = useSendMessage(currentChannel?.id ?? '', user?.id ?? '');

  useEffect(() => {
    if (workspaces.length === 0 && deletedWorkspaces.length === 0) return;
    if (workspaceId) {
      const target =
        workspaces.find((ws) => ws.id === workspaceId) ??
        deletedWorkspaces.find((ws) => ws.id === workspaceId);
      if (target) {
        const needsUpdate =
          currentWorkspace?.id !== target.id || currentWorkspace?.deleted_at !== target.deleted_at;
        if (needsUpdate) selectWorkspace(target);
      } else if (workspaces.length > 0) {
        navigate(`/app/${workspaces[0].id}`, { replace: true });
      }
    } else {
      const ws = currentWorkspace || workspaces[0];
      if (ws) navigate(`/app/${ws.id}`, { replace: true });
    }
  }, [workspaces, deletedWorkspaces, workspaceId, currentWorkspace, selectWorkspace, navigate]);

  useEffect(() => {
    if (!currentWorkspaceId || channels.length === 0) return;
    if (dmUserId) return;
    if (urlChannelId) {
      const target = channels.find((c) => c.id === urlChannelId);
      if (target && currentChannel?.id !== urlChannelId) {
        selectChannel(target);
        markChannelRead(target.id);
        markChannelNotificationsRead(target.id);
      } else if (!target) {
        navigate(`/app/${currentWorkspaceId}`, { replace: true });
      }
    } else {
      const general = channels.find((c) => c.name === 'general') || channels[0];
      navigate(`/app/${currentWorkspaceId}/${general.id}`, { replace: true });
    }
  }, [
    dmUserId,
    urlChannelId,
    channels,
    currentWorkspaceId,
    currentChannel?.id,
    selectChannel,
    markChannelRead,
    markChannelNotificationsRead,
    navigate,
  ]);

  useEffect(() => {
    if (!dmUserId) return;
    if (currentDmPartnerId !== dmUserId) {
      selectDmPartner(dmUserId);
    }
    markDmRead(dmUserId);
    markDmReadServer(dmUserId);
  }, [dmUserId, currentDmPartnerId, selectDmPartner, markDmRead, markDmReadServer]);

  const threadOpenedRef = useRef(false);
  useEffect(() => {
    threadOpenedRef.current = false;
  }, [urlMessageId]);
  const handleTargetMessageFound = useCallback(
    (msg: Message) => {
      if (searchParams.get('thread') === '1' && !threadOpenedRef.current) {
        threadOpenedRef.current = true;
        panel.openThread(msg);
      }
    },
    [searchParams, panel],
  );

  const getWs = useCallback(() => {
    if (currentWorkspace?.instanceUrl) return instanceManager.get(currentWorkspace.instanceUrl).ws;
    return wsClient;
  }, [currentWorkspace?.instanceUrl]);

  useEffect(() => {
    if (!currentWorkspaceId || channels.length === 0) return;
    const ws = getWs();
    channels.forEach((ch) => ws.joinChannel(ch.id));
  }, [currentWorkspaceId, channels, getWs]);

  useEffect(() => {
    requestNotificationPermission();
  }, []);

  useEffect(() => {
    const cleanup = usePresenceStore.getState().initPresenceListener();
    return cleanup;
  }, []);

  const typingTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isTypingRef = useRef(false);

  const handleTyping = useCallback(() => {
    if (!currentChannel) return;
    if (!isTypingRef.current) {
      isTypingRef.current = true;
      getWs().send({ type: 'typing.start', channel_id: currentChannel.id });
    }
    if (typingTimerRef.current) clearTimeout(typingTimerRef.current);
    typingTimerRef.current = setTimeout(() => {
      isTypingRef.current = false;
      if (currentChannel) {
        getWs().send({ type: 'typing.stop', channel_id: currentChannel.id });
      }
    }, 3000);
  }, [currentChannel, getWs]);

  const handleSend = useCallback(
    async (content: string) => {
      if (!currentChannel || !user) return;
      if (typingTimerRef.current) clearTimeout(typingTimerRef.current);
      getWs().send({ type: 'typing.stop', channel_id: currentChannel.id });
      const id = crypto.randomUUID();
      sendMessageMutation.mutate({ content, id });
    },
    [currentChannel, user, getWs, sendMessageMutation],
  );

  const handleFileUpload = useCallback(
    async (file: File) => {
      if (!currentWorkspace || !currentChannel) return;
      setUploading(true);
      try {
        const formData = new FormData();
        formData.append('file', file);
        const uploaded = await getApiForInstance(currentWorkspace.instanceUrl).upload<{
          filename: string;
          url: string;
        }>(`/files/upload/${currentWorkspace.id}`, formData);
        const id = crypto.randomUUID();
        sendMessageMutation.mutate({ content: `[file: ${uploaded.filename}](${uploaded.url})`, id });
      } catch (err) {
        logger.error('WorkspacePage', 'handleFileUpload', err);
        toast.error(ErrorLabels.UploadFailed);
      } finally {
        setUploading(false);
      }
    },
    [currentWorkspace, currentChannel, sendMessageMutation],
  );

  const handleSelectWorkspace = useCallback(
    (ws: { id: string }) => {
      panel.close();
      navigate(ROUTES.workspace(ws.id));
    },
    [panel, navigate],
  );

  const handleSelectChannel = useCallback(
    (ch: Channel) => {
      setMobileNavOpen(false);
      const wsId = workspaceId || currentWorkspace?.id;
      if (!wsId) return;
      navigate(ROUTES.channel(wsId, ch.id));
      const cached = queryClient.getQueryData<InfiniteData<MessagesResponse>>(QUERY_KEYS.messages(ch.id));
      const lastPage = cached?.pages[cached.pages.length - 1];
      const newestMsg = lastPage?.data[lastPage.data.length - 1];
      if (newestMsg) {
        const apiClient = currentWorkspace?.instanceUrl
          ? instanceManager.get(currentWorkspace.instanceUrl).api
          : api;
        apiClient.post(`/channels/${ch.id}/read`, { message_id: newestMsg.id }).catch(() => {});
      }
    },
    [workspaceId, currentWorkspace, navigate, queryClient],
  );

  const handleOpenDm = useCallback(
    (userId: string) => {
      setMobileNavOpen(false);
      const wsId = workspaceId || currentWorkspace?.id;
      if (!wsId) return;
      navigate(ROUTES.dm(wsId, userId));
    },
    [workspaceId, currentWorkspace, navigate],
  );

  const handleNavigateToMessage = useCallback(
    (channelId: string, messageId: string, withThread = false) => {
      panel.close();
      const wsId = workspaceId || currentWorkspace?.id;
      if (!wsId) return;
      const base = ROUTES.message(wsId, channelId, messageId);
      navigate(withThread ? `${base}?thread=1` : base);
    },
    [panel, workspaceId, currentWorkspace, navigate],
  );

  const handleCreateWorkspace = useCallback(
    async (name: string, instanceUrl: string) => {
      const newWs = await createWorkspaceMutation.mutateAsync({ name, instanceUrl });
      await selectWorkspace(newWs);
      navigate(ROUTES.workspace(newWs.id));
    },
    [createWorkspaceMutation, selectWorkspace, navigate],
  );

  const handleCreateChannel = useCallback(
    async (name: string) => {
      if (!currentWorkspace) return;
      await createChannelMutation.mutateAsync({ workspaceId: currentWorkspace.id, name });
    },
    [currentWorkspace, createChannelMutation],
  );

  return {
    user,
    logout,
    navigate,
    workspaces,
    deletedWorkspaces,
    channels,
    workspaceMembers,
    dmConversations,
    currentWorkspace,
    currentChannel,
    unreadChannels,
    mentionChannels,
    mutedChannels,
    currentDmPartnerId,
    unreadDmPartners,
    restoreWorkspace,
    setChannelMuted,
    uploading,
    showProfile,
    setShowProfile,
    showAddInstance,
    setShowAddInstance,
    quickSwitcherOpen,
    setQuickSwitcherOpen,
    mobileNavOpen,
    setMobileNavOpen,
    urlMessageId,
    panel,
    handleTargetMessageFound,
    handleTyping,
    handleSend,
    handleFileUpload,
    handleSelectWorkspace,
    handleSelectChannel,
    handleOpenDm,
    handleNavigateToMessage,
    handleCreateWorkspace,
    handleCreateChannel,
  };
}
