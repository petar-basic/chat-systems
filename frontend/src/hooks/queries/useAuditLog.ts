import { useQuery } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export const useAuditLog = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.auditLog(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace ID');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/audit-log', {
          params: { path: { ws_id: workspaceId }, query: { limit: 200 } },
        }),
      );
      return res.data;
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 15,
  });
};
