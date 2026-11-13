import { useCallback, useEffect, useRef, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useLocation, useMatch, useNavigate, useParams, useSearchParams } from 'react-router';
import { ROUTE_PATTERNS, ROUTES } from '@/shared/constants';
import { messagePath, useMessageLocation } from '@/features/messaging';
import { useChannelPreview } from '@/features/channel';
import { toast } from '@/shared/components/Toast';
import { useWorkspaceStore, type Message, type Workspace, type Channel } from '@/stores/workspace';
import { forgetPlace, lastWorkspaceId, placeIn, rememberPlace } from '../lastPlace';
import type { useRightPanel } from './useRightPanel';

interface Args {
  workspaces: Workspace[];
  deletedWorkspaces: Workspace[];
  noWorkspaces: boolean;
  channels: Channel[];
  conversations: { id: string }[];
  conversationsLoaded: boolean;
  refetchConversations: () => Promise<unknown>;
  markConversationReadServer: (conversationId: string) => void;
  panel: ReturnType<typeof useRightPanel>;
}

interface Place {
  conversationId?: string;
  channelId?: string;
  listId: string | null;
  unreads: boolean;
  threads: boolean;
  people: boolean;
}

function placeOf(workspaceId: string, place: Place): string | null {
  if (place.conversationId) return ROUTES.conversation(workspaceId, place.conversationId);
  if (place.channelId) return ROUTES.channel(workspaceId, place.channelId);
  if (place.listId) return ROUTES.list(workspaceId, place.listId);
  if (place.unreads) return ROUTES.unreads(workspaceId);
  if (place.threads) return ROUTES.threads(workspaceId);
  if (place.people) return ROUTES.people(workspaceId);
  return null;
}

/// The url is the source of truth for which workspace, channel or conversation
/// is open; the store follows it, and an unknown id is redirected rather than
/// left as a blank screen.
export function useWorkspaceRouting({
  workspaces,
  deletedWorkspaces,
  noWorkspaces,
  channels,
  conversations,
  conversationsLoaded,
  refetchConversations,
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
  const { pathname, search } = useLocation();
  const isUnreadsRoute = useMatch(ROUTE_PATTERNS.unreads) !== null;
  const isThreadsRoute = useMatch(ROUTE_PATTERNS.threads) !== null;
  const isPeopleRoute = useMatch(ROUTE_PATTERNS.people) !== null;
  const listId = useMatch(ROUTE_PATTERNS.list)?.params.listId ?? null;
  const {
    currentWorkspace,
    currentChannel,
    currentConversationId,
    selectWorkspace,
    refreshWorkspace,
    selectChannel,
    selectConversation,
    markChannelRead,
    markConversationRead,
  } = useWorkspaceStore(
    useShallow((s) => ({
      currentWorkspace: s.currentWorkspace,
      currentChannel: s.currentChannel,
      currentConversationId: s.currentConversationId,
      selectWorkspace: s.selectWorkspace,
      refreshWorkspace: s.refreshWorkspace,
      selectChannel: s.selectChannel,
      selectConversation: s.selectConversation,
      markChannelRead: s.markChannelRead,
      markConversationRead: s.markConversationRead,
    })),
  );

  // The url is what `channels` was fetched for; the store catches up a render
  // later. Navigating by the store id sends a fresh tab to the workspace it was
  // last on, carrying the new workspace's channel with it.
  const activeWorkspaceId = workspaceId ?? currentWorkspace?.id;

  useEffect(() => {
    if (workspaces.length === 0 && deletedWorkspaces.length === 0) {
      if (noWorkspaces && workspaceId) navigate(ROUTES.app, { replace: true });
      return;
    }
    if (workspaceId) {
      const target =
        workspaces.find((ws) => ws.id === workspaceId) ??
        deletedWorkspaces.find((ws) => ws.id === workspaceId);
      if (target) {
        const needsUpdate =
          currentWorkspace?.id !== target.id || currentWorkspace?.deleted_at !== target.deleted_at;
        if (needsUpdate) selectWorkspace(target);
        else if (currentWorkspace !== target) refreshWorkspace(target);
      } else if (workspaces.length > 0) {
        navigate(`/app/${workspaces[0].id}`, { replace: true });
      }
    } else {
      const last = lastWorkspaceId();
      const ws = currentWorkspace || workspaces.find((w) => w.id === last) || workspaces[0];
      if (ws) navigate(placeIn(ws.id), { replace: true });
    }
  }, [
    workspaces,
    deletedWorkspaces,
    noWorkspaces,
    workspaceId,
    currentWorkspace,
    selectWorkspace,
    refreshWorkspace,
    navigate,
  ]);

  const workspaceReady = !!workspaceId && currentWorkspace?.id === workspaceId;

  const here = workspaceReady
    ? placeOf(workspaceId, {
        conversationId: routeConversationId,
        channelId: urlChannelId,
        listId,
        unreads: isUnreadsRoute,
        threads: isThreadsRoute,
        people: isPeopleRoute,
      })
    : null;

  useEffect(() => {
    if (workspaceId && here) rememberPlace(workspaceId, here);
  }, [workspaceId, here]);
  const { data: place, isError: placeMissing } = useMessageLocation(
    workspaceReady ? urlMessageId : undefined,
  );

  useEffect(() => {
    if (!urlMessageId || !workspaceId) return;
    if (place) {
      const canonical = messagePath(place, urlMessageId);
      if (pathname !== canonical) navigate(`${canonical}${search}`, { replace: true });
    } else if (placeMissing) {
      toast.error('That message was deleted or you can no longer see it');
      const channelId = routeConversationId ?? urlChannelId;
      navigate(
        routeConversationId
          ? ROUTES.conversation(workspaceId, routeConversationId)
          : channelId
            ? ROUTES.channel(workspaceId, channelId)
            : ROUTES.workspace(workspaceId),
        { replace: true },
      );
    }
  }, [
    urlMessageId,
    workspaceId,
    place,
    placeMissing,
    pathname,
    search,
    routeConversationId,
    urlChannelId,
    navigate,
  ]);

  const unlistedChannelId =
    workspaceReady &&
    urlChannelId &&
    !routeConversationId &&
    (!urlMessageId || (place?.kind === 'channel' && place.channel_id === urlChannelId)) &&
    channels.length > 0 &&
    !channels.some((c) => c.id === urlChannelId)
      ? urlChannelId
      : undefined;
  const { data: preview, isError: previewMissing } = useChannelPreview(unlistedChannelId);

  useEffect(() => {
    if (isUnreadsRoute || isThreadsRoute || isPeopleRoute) {
      if (currentChannel?.id || currentConversationId) selectConversation(null);
      return;
    }
    if (listId) return;
    if (!activeWorkspaceId || channels.length === 0) return;
    if (routeConversationId) return;
    if (urlChannelId) {
      const target = channels.find((c) => c.id === urlChannelId);
      if (target) {
        if (currentChannel?.id !== urlChannelId) {
          selectChannel(target);
          markChannelRead(target.id);
        }
      } else if (preview?.id === urlChannelId) {
        if (preview.workspace_id !== activeWorkspaceId) {
          navigate(ROUTES.workspace(activeWorkspaceId), { replace: true });
        } else if (preview.channel_type === 'public' || preview.is_archived) {
          if (currentChannel?.id !== urlChannelId) selectChannel(preview);
        } else if (preview.channel_type === 'private') {
          navigate(ROUTES.workspace(activeWorkspaceId), { replace: true });
        } else {
          navigate(ROUTES.conversation(activeWorkspaceId, urlChannelId), { replace: true });
        }
      } else if (previewMissing) {
        navigate(ROUTES.workspace(activeWorkspaceId), { replace: true });
      }
    } else {
      const general = channels.find((c) => c.name === 'general') || channels[0];
      navigate(`/app/${activeWorkspaceId}/${general.id}`, { replace: true });
    }
  }, [
    isUnreadsRoute,
    isThreadsRoute,
    isPeopleRoute,
    listId,
    currentConversationId,
    selectConversation,
    routeConversationId,
    urlChannelId,
    preview,
    previewMissing,
    channels,
    activeWorkspaceId,
    currentChannel?.id,
    selectChannel,
    markChannelRead,
    navigate,
  ]);

  const unlistedConversationId =
    workspaceReady &&
    routeConversationId &&
    conversationsLoaded &&
    !conversations.some((c) => c.id === routeConversationId)
      ? routeConversationId
      : undefined;
  const refetchedFor = useRef<string | null>(null);
  const [checkedConversation, setCheckedConversation] = useState<string | null>(null);
  useEffect(() => {
    if (!unlistedConversationId || !workspaceId) return;
    if (channels.some((c) => c.id === unlistedConversationId)) {
      forgetPlace(workspaceId, ROUTES.conversation(workspaceId, unlistedConversationId));
      navigate(ROUTES.channel(workspaceId, unlistedConversationId), { replace: true });
      return;
    }
    if (checkedConversation === unlistedConversationId) {
      forgetPlace(workspaceId, ROUTES.conversation(workspaceId, unlistedConversationId));
      navigate(ROUTES.workspace(workspaceId), { replace: true });
      return;
    }
    if (refetchedFor.current === unlistedConversationId) return;
    refetchedFor.current = unlistedConversationId;
    void refetchConversations().then(() => setCheckedConversation(unlistedConversationId));
  }, [unlistedConversationId, checkedConversation, workspaceId, channels, refetchConversations, navigate]);

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
  const replyId = place?.thread_parent_id ? urlMessageId : undefined;
  const handleTargetMessageFound = useCallback(
    (msg: Message) => {
      if ((replyId || searchParams.get('thread') === '1') && !threadOpenedRef.current) {
        threadOpenedRef.current = true;
        panel.openThread(msg, replyId);
      }
    },
    [replyId, searchParams, panel],
  );

  const highlightMessageId = urlMessageId && place ? (place.thread_parent_id ?? urlMessageId) : undefined;
  const previewing =
    !!currentChannel && channels.length > 0 && !channels.some((c) => c.id === currentChannel.id);

  return {
    highlightMessageId,
    handleTargetMessageFound,
    previewing,
    isUnreadsRoute,
    isThreadsRoute,
    isPeopleRoute,
    listId,
  };
}
