import type { QueryClient } from '@tanstack/react-query';
import { useWorkspaceStore } from '@/stores/workspace';

type Listed = Array<{ id: string }>;

function ownerIn(queryClient: QueryClient, prefix: string, at: number, channelId: string): string | null {
  for (const [key, rows] of queryClient.getQueriesData<Listed>({ queryKey: [prefix] })) {
    if (prefix === 'workspaces' && key[2] !== 'channels') continue;
    if (Array.isArray(rows) && rows.some((row) => row.id === channelId)) return String(key[at]);
  }
  return null;
}

export function workspaceOfChannel(queryClient: QueryClient, channelId: string): string | null {
  return (
    ownerIn(queryClient, 'workspaces', 1, channelId) ?? ownerIn(queryClient, 'conversations', 1, channelId)
  );
}

export function belongsElsewhere(queryClient: QueryClient, channelId: string): boolean {
  const owner = workspaceOfChannel(queryClient, channelId);
  return owner !== null && owner !== useWorkspaceStore.getState().currentWorkspace?.id;
}
