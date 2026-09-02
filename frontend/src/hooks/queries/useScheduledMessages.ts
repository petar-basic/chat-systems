import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type ScheduledMessage = components['schemas']['ScheduledMessage'];

export interface ScheduleTarget {
  channelId: string;
}

export const useScheduledMessages = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.scheduledMessages(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/scheduled-messages', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.data;
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: 1000 * 30,
  });
};

export const useScheduleMessage = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      target,
      content,
      sendAt,
    }: {
      target: ScheduleTarget;
      content: string;
      sendAt: Date;
    }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/scheduled-messages', {
          params: { path: { ws_id: workspaceId } },
          body: {
            channel_id: target.channelId,
            content,
            send_at: sendAt.toISOString(),
          },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.scheduledMessages(workspaceId) });
    },
  });
};

export const useRescheduleMessage = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, sendAt }: { id: string; sendAt: Date }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PATCH('/scheduled-messages/{id}', {
          params: { path: { id } },
          body: { send_at: sendAt.toISOString() },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.scheduledMessages(workspaceId) });
    },
  });
};

export const useCancelScheduledMessage = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/scheduled-messages/{id}', { params: { path: { id } } }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.scheduledMessages(workspaceId) });
    },
  });
};
