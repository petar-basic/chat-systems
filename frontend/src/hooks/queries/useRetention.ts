import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance, useCurrentApi } from '@/shared/hooks/useCurrentApi';
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

export interface RetentionDraft {
  messageDays: number | null;
  fileDays: number | null;
  channelOverrides: boolean;
}

export const useRetentionPreview = (workspaceId: string, draft: RetentionDraft, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.retentionPreview(workspaceId, draft),
    queryFn: () =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/retention/preview', {
          params: {
            path: { ws_id: workspaceId },
            query: {
              message_days: draft.messageDays ?? undefined,
              file_days: draft.fileDays ?? undefined,
              channel_overrides: draft.channelOverrides || undefined,
            },
          },
        }),
      ),
    enabled: draft.messageDays !== null || draft.fileDays !== null || draft.channelOverrides,
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

export type ChannelRetention = components['schemas']['ChannelRetention'];
export type ChannelPeriod = number | null | 'workspace';

export const useChannelRetention = (channelId: string) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.channelRetention(channelId),
    queryFn: () =>
      apiClient.typed((c) =>
        c.GET('/channels/{ch_id}/retention', { params: { path: { ch_id: channelId } } }),
      ),
  });
};

export const useSetChannelRetention = (channelId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: (period: ChannelPeriod) =>
      apiClient.typed((c) =>
        period === 'workspace'
          ? c.DELETE('/channels/{ch_id}/retention', { params: { path: { ch_id: channelId } } })
          : c.PUT('/channels/{ch_id}/retention', {
              params: { path: { ch_id: channelId } },
              body: { message_days: period },
            }),
      ),
    onSuccess: (res) => queryClient.setQueryData(QUERY_KEYS.channelRetention(channelId), res),
  });
};
