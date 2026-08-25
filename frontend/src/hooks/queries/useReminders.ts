import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export interface Reminder {
  id: string;
  workspace_id: string;
  created_by: string;
  target_user_id: string;
  channel_id: string | null;
  message_id: string | null;
  content: string;
  remind_at: string;
  is_delivered: boolean;
  created_at: string;
}

export const useReminders = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.reminders(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: Reminder[] }>(
        `/workspaces/${workspaceId}/reminders`,
      );
      return res.data;
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: 1000 * 30,
  });
};

export const useCreateReminder = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      targetUserId,
      content,
      remindAt,
      channelId,
      messageId,
    }: {
      targetUserId: string;
      content: string;
      remindAt: Date;
      channelId?: string | null;
      messageId?: string | null;
    }) =>
      getApiForInstance(instanceUrl).post<Reminder>(`/workspaces/${workspaceId}/reminders`, {
        target_user_id: targetUserId,
        content,
        remind_at: remindAt.toISOString(),
        channel_id: channelId ?? null,
        message_id: messageId ?? null,
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.reminders(workspaceId) });
    },
  });
};

export const useCancelReminder = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (reminderId: string) =>
      getApiForInstance(instanceUrl).delete(`/workspaces/${workspaceId}/reminders/${reminderId}`),
    onMutate: async (reminderId) => {
      await queryClient.cancelQueries({ queryKey: QUERY_KEYS.reminders(workspaceId) });
      const previous = queryClient.getQueryData<Reminder[]>(QUERY_KEYS.reminders(workspaceId));
      queryClient.setQueryData<Reminder[]>(QUERY_KEYS.reminders(workspaceId), (old) =>
        old?.filter((reminder) => reminder.id !== reminderId),
      );
      return { previous };
    },
    onError: (_err, _reminderId, ctx) => {
      if (ctx?.previous) queryClient.setQueryData(QUERY_KEYS.reminders(workspaceId), ctx.previous);
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.reminders(workspaceId) });
    },
  });
};
