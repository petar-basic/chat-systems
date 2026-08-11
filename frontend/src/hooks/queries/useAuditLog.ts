import { useQuery } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import type { AuditEntry } from '@/features/workspace/components/AuditLogTable';

export const useAuditLog = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.auditLog(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: AuditEntry[] }>(
        `/workspaces/${workspaceId}/audit-log?limit=200`,
      );
      return res.data;
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 15,
  });
};
