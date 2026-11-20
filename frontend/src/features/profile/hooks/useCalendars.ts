import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { useWorkspaceStore } from '@/stores/workspace';
import { QUERY_KEYS, ErrorLabels } from '@/shared/constants';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { logger } from '@/lib/logger';

export type CalendarState = components['schemas']['CalendarState'];
type CalendarList = components['schemas']['CalendarList'];
type CalendarChanges = components['schemas']['UpdateCalendarSettings'];

export function useCalendars() {
  const apiClient = useCurrentApi();
  const ws = useWorkspaceStore((s) => s.currentWorkspace?.id) ?? '';
  return useQuery({
    queryKey: QUERY_KEYS.calendars(ws),
    enabled: !!ws,
    queryFn: () =>
      apiClient.typed((c) => c.GET('/workspaces/{ws_id}/calendars', { params: { path: { ws_id: ws } } })),
  });
}

export function useUpdateCalendar() {
  const apiClient = useCurrentApi();
  const queryClient = useQueryClient();
  const ws = useWorkspaceStore((s) => s.currentWorkspace?.id) ?? '';
  const key = QUERY_KEYS.calendars(ws);
  const replace = (list: CalendarList | undefined, next: CalendarState) =>
    list && { data: list.data.map((c) => (c.provider === next.provider ? next : c)) };

  return useMutation({
    mutationFn: ({ provider, changes }: { provider: CalendarState['provider']; changes: CalendarChanges }) =>
      apiClient.typed((c) =>
        c.PATCH('/workspaces/{ws_id}/calendars/{provider}', {
          params: { path: { ws_id: ws, provider } },
          body: changes,
        }),
      ),
    onMutate: async ({ provider, changes }) => {
      await queryClient.cancelQueries({ queryKey: key });
      const before = queryClient.getQueryData<CalendarList>(key);
      const current = before?.data.find((c) => c.provider === provider);
      const settings = current?.settings;
      if (current && settings) {
        const changed = Object.fromEntries(Object.entries(changes).filter(([, value]) => value != null));
        queryClient.setQueryData<CalendarList>(key, (list) =>
          replace(list, { ...current, settings: { ...settings, ...changed } }),
        );
      }
      return { before };
    },
    onError: (err, _vars, context) => {
      if (context?.before) queryClient.setQueryData(key, context.before);
      logger.error('useUpdateCalendar', 'update', err);
      toast.error(toUserMessage(err, ErrorLabels.CalendarSaveFailed));
    },
    onSuccess: (saved) => queryClient.setQueryData<CalendarList>(key, (list) => replace(list, saved)),
  });
}
