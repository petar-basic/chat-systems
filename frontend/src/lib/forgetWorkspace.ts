import type { QueryClient } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';

export function forgetWorkspace(queryClient: QueryClient, workspaceId: string) {
  queryClient.setQueriesData<Workspace[]>(
    {
      queryKey: QUERY_KEYS.workspaces(),
      predicate: (query) => query.queryKey.length === 2 && Array.isArray(query.state.data),
    },
    (list) => list?.filter((ws) => ws.id !== workspaceId),
  );
  queryClient.removeQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
  queryClient.removeQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });
  if (useWorkspaceStore.getState().currentWorkspace?.id === workspaceId) {
    useWorkspaceStore.setState({ currentWorkspace: null, currentChannel: null, currentConversationId: null });
  }
}
