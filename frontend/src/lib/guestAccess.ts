import type { QueryClient } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import { toast } from '@/shared/components/Toast';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';

function workspaceName(queryClient: QueryClient, workspaceId: string): string | undefined {
  const current = useWorkspaceStore.getState().currentWorkspace;
  if (current?.id === workspaceId) return current.name;
  return queryClient
    .getQueriesData<Workspace[]>({ queryKey: QUERY_KEYS.workspaces() })
    .flatMap(([, list]) => (Array.isArray(list) ? list : []))
    .find((ws) => ws.id === workspaceId)?.name;
}

export function sayGuestAccessEnded(queryClient: QueryClient, workspaceId: string) {
  const workspace = workspaceName(queryClient, workspaceId);
  const message = workspace
    ? i18n.t('workspace:guest.accessEnded', { workspace })
    : i18n.t('workspace:guest.accessEndedUnnamed');
  toast.info(message, { durationMs: 0 });
}
