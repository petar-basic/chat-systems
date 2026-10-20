import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type Reminder = components['schemas']['Reminder'];

export const useReminders = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.reminders(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/reminders', { params: { path: { ws_id: workspaceId } } }),
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
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/reminders', {
          params: { path: { ws_id: workspaceId } },
          body: {
            target_user_id: targetUserId,
            content,
            remind_at: remindAt.toISOString(),
            channel_id: channelId ?? null,
            message_id: messageId ?? null,
          },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.reminders(workspaceId) });
    },
  });
};

export const useCancelReminder = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (reminderId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/workspaces/{ws_id}/reminders/{reminder_id}', {
          params: { path: { ws_id: workspaceId, reminder_id: reminderId } },
        }),
      ),
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
