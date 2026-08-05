import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export interface ScheduledMessage {
  id: string;
  workspace_id: string;
  user_id: string;
  channel_id: string | null;
  conversation_id: string | null;
  content: string;
  send_at: string;
  sent_at: string | null;
  canceled_at: string | null;
  created_at: string;
}

export interface ScheduleTarget {
  channelId?: string;
  conversationId?: string;
}

export const useScheduledMessages = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.scheduledMessages(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: ScheduledMessage[] }>(
        `/workspaces/${workspaceId}/scheduled-messages`,
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
      getApiForInstance(instanceUrl).post<ScheduledMessage>(`/workspaces/${workspaceId}/scheduled-messages`, {
        channel_id: target.channelId ?? null,
        conversation_id: target.conversationId ?? null,
        content,
        send_at: sendAt.toISOString(),
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.scheduledMessages(workspaceId) });
    },
  });
};

export const useCancelScheduledMessage = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => getApiForInstance(instanceUrl).delete(`/scheduled-messages/${id}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.scheduledMessages(workspaceId) });
    },
  });
};
