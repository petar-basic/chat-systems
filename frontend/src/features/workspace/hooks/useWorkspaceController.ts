import { useCallback, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { CreateChannelDraft } from '@/shared/models/channel';
import { useNavigate, useParams } from 'react-router';
import { useQueryClient } from '@tanstack/react-query';
import { useCurrentUser, useLogout } from '@/features/instance';
import { useWorkspaceStore, type Channel } from '@/stores/workspace';
import { ROUTES } from '@/shared/constants';
import { useCreateWorkspace, useCreateChannel } from '@/features/workspace/hooks/useWorkspaces';
import { useOpenConversation, useOpenMessage } from '@/features/messaging';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { newestSentIn } from '@/lib/readOnScreen';
import { useRightPanel } from './useRightPanel';
import { useWorkspaceData } from './useWorkspaceData';
import { useWorkspaceRouting } from './useWorkspaceRouting';
import { useComposer } from './useComposer';
import { useWorkspaceShortcuts } from './useWorkspaceShortcuts';
import { useMarkWorkspaceRead } from '@/features/unreads';
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

  const [accountDialog, setAccountDialog] = useState<'profile' | 'preferences' | null>(null);
  const [showAddInstance, setShowAddInstance] = useState(false);
  const [quickSwitcherOpen, setQuickSwitcherOpen] = useState(false);
  const [mobileNavOpen, setMobileNavOpen] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);

  const panel = useRightPanel(currentChannel?.id, currentConversationId);

  const { workspaceId } = useParams<{ workspaceId?: string }>();
  const data = useWorkspaceData(workspaceId, user?.id);
  const routing = useWorkspaceRouting({
    workspaces: data.workspaces,
    deletedWorkspaces: data.deletedWorkspaces,
    channels: data.channels,
    markConversationReadServer: data.markConversationReadServer,
    panel,
  });

  const unreadsWorkspaceId = workspaceId ?? currentWorkspace?.id;
  const handleOpenUnreads = useCallback(() => {
    if (!unreadsWorkspaceId) return;
    setMobileNavOpen(false);
    navigate(ROUTES.unreads(unreadsWorkspaceId));
  }, [navigate, unreadsWorkspaceId]);
  const handleOpenThreads = useCallback(() => {
    if (!unreadsWorkspaceId) return;
    setMobileNavOpen(false);
    navigate(ROUTES.threads(unreadsWorkspaceId));
  }, [navigate, unreadsWorkspaceId]);

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

  const markChannelReadServer = useCallback(
    (channelId: string) => {
      const newest = newestSentIn(queryClient, channelId);
      if (!newest) return;
      getApiForInstance(currentWorkspace?.instanceUrl)
        .typed((c) =>
          c.POST('/channels/{ch_id}/read', {
            params: { path: { ch_id: channelId } },
            body: { message_id: newest },
          }),
        )
        .catch(() => {});
    },
    [queryClient, currentWorkspace],
  );

  const handleSelectChannel = useCallback(
    (ch: Channel) => {
      setMobileNavOpen(false);
      const wsId = workspaceId || currentWorkspace?.id;
      if (!wsId) return;
      navigate(ROUTES.channel(wsId, ch.id));
    },
    [workspaceId, currentWorkspace, navigate],
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

  const openMessage = useOpenMessage();
  const handleOpenMessage = useCallback(
    (messageId: string, thread = false) => {
      panel.close();
      void openMessage(messageId, thread);
    },
    [panel, openMessage],
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

  const markWorkspaceRead = useMarkWorkspaceRead(unreadsWorkspaceId);
  const markChannelRead = useWorkspaceStore((s) => s.markChannelRead);
  useWorkspaceShortcuts({
    quickSwitcher: () => setQuickSwitcherOpen((open) => !open),
    shortcuts: () => setShortcutsOpen((open) => !open),
    allUnreads: handleOpenUnreads,
    markChannelRead: () => {
      if (currentChannel) {
        markChannelRead(currentChannel.id);
        markChannelReadServer(currentChannel.id);
      } else if (currentConversationId) {
        data.markConversationReadServer(currentConversationId);
      }
    },
    markAllRead: () => {
      if (unreadsWorkspaceId) markWorkspaceRead.mutate();
    },
  });

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
    channelsPending: data.channelsPending,
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
    accountDialog,
    setAccountDialog,
    showAddInstance,
    setShowAddInstance,
    quickSwitcherOpen,
    setQuickSwitcherOpen,
    mobileNavOpen,
    setMobileNavOpen,
    shortcutsOpen,
    setShortcutsOpen,
    highlightMessageId: routing.highlightMessageId,
    previewing: routing.previewing,
    isUnreadsRoute: routing.isUnreadsRoute,
    handleOpenUnreads,
    isThreadsRoute: routing.isThreadsRoute,
    handleOpenThreads,
    listId: routing.listId,
    panel,
    handleTargetMessageFound: routing.handleTargetMessageFound,
    handleSelectWorkspace,
    handleSelectChannel,
    handleOpenConversation,
    handleOpenWith,
    handleOpenMessage,
    handleCreateWorkspace,
    handleCreateChannel,
  };
}
