import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore } from '@/stores/workspace';

export type NotificationSchedule = components['schemas']['NotificationSchedule'];
export type ScheduleChange = components['schemas']['SetScheduleRequest'];

export const useNotificationSchedule = () => {
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const workspaceId = workspace?.id ?? '';
  return useQuery({
    queryKey: QUERY_KEYS.notificationSchedule(workspaceId),
    queryFn: () =>
      getApiForInstance(workspace?.instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/notifications/schedule', { params: { path: { ws_id: workspaceId } } }),
      ),
    enabled: !!workspace,
    staleTime: 1000 * 60 * 5,
  });
};

export const useSetNotificationSchedule = () => {
  const queryClient = useQueryClient();
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const workspaceId = workspace?.id ?? '';
  const key = QUERY_KEYS.notificationSchedule(workspaceId);
  return useMutation({
    mutationFn: (change: ScheduleChange) =>
      getApiForInstance(workspace?.instanceUrl).typed((c) =>
        c.PUT('/workspaces/{ws_id}/notifications/schedule', {
          params: { path: { ws_id: workspaceId } },
          body: change,
        }),
      ),
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
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.notificationDnd(workspaceId) });
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });
    },
  });
};
