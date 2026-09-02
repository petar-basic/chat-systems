import { useCallback, useEffect, useRef } from 'react';
import { useNavigate, useParams, useSearchParams } from 'react-router';
import { useWorkspaceStore, type Message, type Workspace, type Channel } from '@/stores/workspace';
import type { useRightPanel } from './useRightPanel';

interface Args {
  workspaces: Workspace[];
  deletedWorkspaces: Workspace[];
  channels: Channel[];
  markChannelNotificationsRead: (channelId: string) => void;
  markConversationReadServer: (conversationId: string) => void;
  panel: ReturnType<typeof useRightPanel>;
}

/// The url is the source of truth for which workspace, channel or conversation
/// is open; the store follows it, and an unknown id is redirected rather than
/// left as a blank screen.
export function useWorkspaceRouting({
  workspaces,
  deletedWorkspaces,
  channels,
  markChannelNotificationsRead,
  markConversationReadServer,
  panel,
}: Args) {
  const navigate = useNavigate();
  const {
    workspaceId,
    channelId: urlChannelId,
    messageId: urlMessageId,
    conversationId: routeConversationId,
  } = useParams<{ workspaceId?: string; channelId?: string; messageId?: string; conversationId?: string }>();
  const [searchParams] = useSearchParams();
  const {
    currentWorkspace,
    currentChannel,
    currentConversationId,
    selectWorkspace,
    selectChannel,
    selectConversation,
    markChannelRead,
    markConversationRead,
  } = useWorkspaceStore();

  // The url is what `channels` was fetched for; the store catches up a render
  // later. Navigating by the store id sends a fresh tab to the workspace it was
  // last on, carrying the new workspace's channel with it.
  const activeWorkspaceId = workspaceId ?? currentWorkspace?.id;

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
    if (!activeWorkspaceId || channels.length === 0) return;
    if (routeConversationId) return;
    if (urlChannelId) {
      const target = channels.find((c) => c.id === urlChannelId);
      if (target && currentChannel?.id !== urlChannelId) {
        selectChannel(target);
        markChannelRead(target.id);
        markChannelNotificationsRead(target.id);
      } else if (!target) {
        navigate(`/app/${activeWorkspaceId}`, { replace: true });
      }
    } else {
      const general = channels.find((c) => c.name === 'general') || channels[0];
      navigate(`/app/${activeWorkspaceId}/${general.id}`, { replace: true });
    }
  }, [
    routeConversationId,
    urlChannelId,
    channels,
    activeWorkspaceId,
    currentChannel?.id,
    selectChannel,
    markChannelRead,
    markChannelNotificationsRead,
    navigate,
  ]);

  useEffect(() => {
    if (!routeConversationId) return;
    if (currentConversationId !== routeConversationId) {
      selectConversation(routeConversationId);
    }
    markConversationRead(routeConversationId);
    markConversationReadServer(routeConversationId);
  }, [
    routeConversationId,
    currentConversationId,
    selectConversation,
    markConversationRead,
    markConversationReadServer,
  ]);

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

  return { urlMessageId, handleTargetMessageFound };
}
