import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type NotificationSchedule = components['schemas']['NotificationSchedule'];
export type ScheduleChange = components['schemas']['SetScheduleRequest'];

export const useNotificationSchedule = () => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.notificationSchedule(),
    queryFn: () => apiClient.typed((c) => c.GET('/users/me/notification-schedule')),
    staleTime: 1000 * 60 * 5,
  });
};

export const useSetNotificationSchedule = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  const key = QUERY_KEYS.notificationSchedule();
  return useMutation({
    mutationFn: (change: ScheduleChange) =>
      apiClient.typed((c) => c.PUT('/users/me/notification-schedule', { body: change })),
    onMutate: (change) => {
      const previous = queryClient.getQueryData<NotificationSchedule>(key);
      if (previous) queryClient.setQueryData<NotificationSchedule>(key, { ...previous, ...change });
      return { previous };
    },
    onError: (_err, _change, context) => {
      if (context?.previous) queryClient.setQueryData(key, context.previous);
    },
    onSuccess: (saved) => {
      queryClient.setQueryData(key, saved);
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.notificationDnd() });
    },
  });
};
