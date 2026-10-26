import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type RetentionPolicy = components['schemas']['RetentionPolicy'];
export type RetentionChange = components['schemas']['UpdateRetentionRequest'];

export const useRetentionPolicy = (workspaceId: string, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.retention(workspaceId),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/retention', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.policy ?? null;
    },
  });

export const useRetentionPreview = (
  workspaceId: string,
  messageDays: number | null,
  fileDays: number | null,
  instanceUrl?: string,
) =>
  useQuery({
    queryKey: QUERY_KEYS.retentionPreview(workspaceId, messageDays, fileDays),
    queryFn: () =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/retention/preview', {
          params: {
            path: { ws_id: workspaceId },
            query: { message_days: messageDays ?? undefined, file_days: fileDays ?? undefined },
          },
        }),
      ),
    enabled: messageDays !== null || fileDays !== null,
  });

export const useSaveRetention = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: RetentionChange) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PATCH('/workspaces/{ws_id}/retention', { params: { path: { ws_id: workspaceId } }, body }),
      ),
    onSuccess: (res) => queryClient.setQueryData(QUERY_KEYS.retention(workspaceId), res.policy ?? null),
  });
};
