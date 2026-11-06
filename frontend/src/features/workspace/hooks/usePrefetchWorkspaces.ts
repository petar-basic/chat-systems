import { useEffect, useRef } from 'react';
import { useQueryClient, type QueryClient } from '@tanstack/react-query';
import { conversationsQuery, prefetchMessages } from '@/features/messaging';
import type { Workspace } from '@/stores/workspace';
import { rememberedTarget } from '../lastPlace';
import { workspaceChannelsQuery, workspaceMembersQuery } from './useWorkspaces';

const PREFETCH_LIMIT = 10;
const SETTLE_MS = 1500;

async function prefetchWorkspace(queryClient: QueryClient, ws: Workspace) {
  const [channels, conversations] = await Promise.all([
    queryClient.fetchQuery(workspaceChannelsQuery(ws.id, ws.instanceUrl)),
    queryClient.fetchQuery(conversationsQuery(ws.id, ws.instanceUrl)),
    queryClient.prefetchQuery(workspaceMembersQuery(ws.id, ws.instanceUrl)),
  ]);
  const target = rememberedTarget(ws.id);
  const channelId = !target
    ? (channels.find((c) => c.name === 'general') ?? channels[0])?.id
    : 'conversationId' in target
      ? conversations.find((c) => c.id === target.conversationId)?.id
      : channels.find((c) => c.id === target.channelId)?.id;
  if (channelId) await prefetchMessages(queryClient, channelId, ws.instanceUrl);
}

export function usePrefetchWorkspaces(
  workspaces: Workspace[],
  currentWorkspaceId: string | undefined,
  ready: boolean,
) {
  const queryClient = useQueryClient();
  const started = useRef(false);

  useEffect(() => {
    if (!ready || started.current || !currentWorkspaceId || workspaces.length < 2) return undefined;
    started.current = true;
    const others = workspaces.filter((ws) => ws.id !== currentWorkspaceId).slice(0, PREFETCH_LIMIT);
    const timer = setTimeout(() => {
      void (async () => {
        for (const ws of others) await prefetchWorkspace(queryClient, ws).catch(() => undefined);
      })();
    }, SETTLE_MS);
    return () => clearTimeout(timer);
  }, [ready, currentWorkspaceId, workspaces, queryClient]);
}
