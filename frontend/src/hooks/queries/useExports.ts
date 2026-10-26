import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type ExportDetail = components['schemas']['ExportDetail'];

function inProgress(rows: ExportDetail[]) {
  return rows.some((row) => row.export.status === 'pending' || row.export.status === 'running');
}

export const useExports = (workspaceId: string, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.exports(workspaceId),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/exports', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.data;
    },
    refetchInterval: (query) => (inProgress(query.state.data ?? []) ? 2000 : false),
  });

export const useStartWorkspaceExport = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (includeDms: boolean) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/exports', {
          params: { path: { ws_id: workspaceId } },
          body: { include_dms: includeDms },
        }),
      ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: QUERY_KEYS.exports(workspaceId) }),
  });
};

export const useStartPersonExport = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (userId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/users/{user_id}/exports', { params: { path: { user_id: userId } }, body: {} }),
      ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: QUERY_KEYS.exports(workspaceId) }),
  });
};

export const useErasePerson = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (userId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/admin/users/{user_id}/data', {
          params: { path: { user_id: userId } },
          body: { hard_delete: false },
        }),
      ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) }),
  });
};
