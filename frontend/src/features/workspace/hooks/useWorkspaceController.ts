import { useCallback, useEffect, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { CreateChannelDraft } from '@/models/channel';
import { useNavigate, useParams } from 'react-router';
import { useQueryClient } from '@tanstack/react-query';
import { useCurrentUser, useLogout } from '@/hooks/queries/useAuth';
import { useWorkspaceStore, type Channel } from '@/stores/workspace';
import { ROUTES, QUERY_KEYS } from '@/shared/constants';
import { useCreateWorkspace, useCreateChannel } from '@/hooks/queries/useWorkspaces';
import { useOpenConversation } from '@/hooks/queries/useConversations';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import type { MessagesInfiniteData } from '@/hooks/queries/useMessages';
import { useRightPanel } from './useRightPanel';
import { useWorkspaceData } from './useWorkspaceData';
import { useWorkspaceRouting } from './useWorkspaceRouting';
import { useComposer } from './useComposer';
import { useMessageActions } from './useMessageActions';

export function useWorkspaceController() {
  const { data: user } = useCurrentUser();
  const logout = useLogout();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const {
    currentWorkspace,
    currentChannel,
    unreadChannels,
    mentionChannels,
    unreadCounts,
    mentionCounts,
    mutedChannels,
    currentConversationId,
    unreadConversations,
    selectWorkspace,
  } = useWorkspaceStore(
    useShallow((s) => ({
      currentWorkspace: s.currentWorkspace,
      currentChannel: s.currentChannel,
      unreadChannels: s.unreadChannels,
      mentionChannels: s.mentionChannels,
      unreadCounts: s.unreadCounts,
      mentionCounts: s.mentionCounts,
      mutedChannels: s.mutedChannels,
      currentConversationId: s.currentConversationId,
      unreadConversations: s.unreadConversations,
      selectWorkspace: s.selectWorkspace,
    })),
  );

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

  const panel = useRightPanel(currentChannel?.id, currentConversationId);

  const { workspaceId } = useParams<{ workspaceId?: string }>();
  const data = useWorkspaceData(workspaceId, user?.id);
  const routing = useWorkspaceRouting({
    workspaces: data.workspaces,
    deletedWorkspaces: data.deletedWorkspaces,
    channels: data.channels,
    markChannelNotificationsRead: data.markChannelNotificationsRead,
    markConversationReadServer: data.markConversationReadServer,
    panel,
  });

  const composer = useComposer({
    currentWorkspace,
    currentChannel,
    userId: user?.id,
    currentWsInstanceUrl: data.currentWsInstanceUrl,
  });

  const createWorkspaceMutation = useCreateWorkspace();
  const createChannelMutation = useCreateChannel();
  const openConversation = useOpenConversation(workspaceId ?? '', data.currentWsInstanceUrl);

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
      const cached = queryClient.getQueryData<MessagesInfiniteData>(QUERY_KEYS.messages(ch.id));
      const lastPage = cached?.pages[cached.pages.length - 1];
      const newestMsg = lastPage?.data[lastPage.data.length - 1];
      if (newestMsg) {
        getApiForInstance(currentWorkspace?.instanceUrl)
          .typed((c) =>
            c.POST('/channels/{ch_id}/read', {
              params: { path: { ch_id: ch.id } },
              body: { message_id: newestMsg.id },
            }),
          )
          .catch(() => {});
      }
    },
    [workspaceId, currentWorkspace, navigate, queryClient],
  );

  const handleOpenConversation = useCallback(
    (conversationId: string) => {
      setMobileNavOpen(false);
      const wsId = workspaceId || currentWorkspace?.id;
      if (!wsId) return;
      navigate(ROUTES.conversation(wsId, conversationId));
    },
    [workspaceId, currentWorkspace, navigate],
  );

  const handleOpenWith = useCallback(
    async (participantIds: string[]) => {
      const wsId = workspaceId || currentWorkspace?.id;
      if (!wsId || participantIds.length === 0) return;
      const conversation = await openConversation.mutateAsync(participantIds);
      handleOpenConversation(conversation.id);
    },
    [workspaceId, currentWorkspace, openConversation, handleOpenConversation],
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
    async (draft: CreateChannelDraft) => {
      if (!currentWorkspace) return;
      const created = await createChannelMutation.mutateAsync({
        workspaceId: currentWorkspace.id,
        name: draft.name,
        type: draft.isPrivate ? 'private' : 'public',
        description: draft.description,
        postPolicy: draft.announcementOnly ? 'moderators' : undefined,
      });
      handleSelectChannel(created);
    },
    [currentWorkspace, createChannelMutation, handleSelectChannel],
  );

  const actions = useMessageActions({
    workspaceId,
    currentWsInstanceUrl: data.currentWsInstanceUrl,
    channels: data.channels,
    conversations: data.conversations,
    userId: user?.id,
    getUser: data.getUser,
  });

  return {
    ...composer,
    ...actions,
    user,
    logout,
    navigate,
    workspaces: data.workspaces,
    deletedWorkspaces: data.deletedWorkspaces,
    channels: data.channels,
    workspaceMembers: data.workspaceMembers,
    conversations: data.conversations,
    currentWorkspace,
    currentChannel,
    unreadChannels,
    mentionChannels,
    unreadCounts,
    mentionCounts,
    mutedChannels,
    currentConversationId,
    unreadConversations,
    restoreWorkspace: data.restoreWorkspace,
    setChannelMuted: data.setChannelMuted,
    showProfile,
    setShowProfile,
    showAddInstance,
    setShowAddInstance,
    quickSwitcherOpen,
    setQuickSwitcherOpen,
    mobileNavOpen,
    setMobileNavOpen,
    urlMessageId: routing.urlMessageId,
    panel,
    handleTargetMessageFound: routing.handleTargetMessageFound,
    handleSelectWorkspace,
    handleSelectChannel,
    handleOpenConversation,
    handleOpenWith,
    handleNavigateToMessage,
    handleCreateWorkspace,
    handleCreateChannel,
  };
}
