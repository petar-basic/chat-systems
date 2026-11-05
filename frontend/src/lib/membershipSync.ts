import type { QueryClient } from '@tanstack/react-query';
import { globalEventBus } from './globalEventBus';
import { QUERY_KEYS } from '@/shared/constants';

export function syncMembershipFrames(queryClient: QueryClient): Array<() => void> {
  return [
    globalEventBus.on('workspace.joined', () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
    }),
    globalEventBus.on('workspace.members_changed', (event) => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(event.workspace_id) });
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.conversations(event.workspace_id) });
      queryClient.invalidateQueries({
        predicate: ({ queryKey }) => queryKey[0] === 'channels' && queryKey[2] === 'members',
      });
    }),
  ];
}
