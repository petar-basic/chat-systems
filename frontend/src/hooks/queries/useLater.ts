import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS, ErrorLabels } from '@/shared/constants';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { logger } from '@/lib/logger';

export type LaterItem = components['schemas']['LaterItem'];
export type LaterStatus = components['schemas']['LaterStatus'];

function laterQuery(workspaceId: string | undefined, status: LaterStatus, instanceUrl?: string) {
  return {
    queryKey: QUERY_KEYS.later(workspaceId ?? '', status),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/later', {
          params: { path: { ws_id: workspaceId ?? '' }, query: { status } },
        }),
      );
      return res.data;
    },
    enabled: !!workspaceId,
    staleTime: 30_000,
  };
}

export function useLater(workspaceId: string | undefined, status: LaterStatus, instanceUrl?: string) {
  return useQuery(laterQuery(workspaceId, status, instanceUrl));
}

export function useSavedItemId(workspaceId: string | undefined, messageId: string, instanceUrl?: string) {
  return useQuery({
    ...laterQuery(workspaceId, 'in_progress', instanceUrl),
    select: (items: LaterItem[]) => items.find((item) => item.message?.id === messageId)?.id,
  }).data;
}

export function useLaterActions(workspaceId: string | undefined, instanceUrl?: string) {
  const queryClient = useQueryClient();
  const ws = workspaceId ?? '';
  const api = () => getApiForInstance(instanceUrl);
  const refresh = () => queryClient.invalidateQueries({ queryKey: QUERY_KEYS.later(ws) });
  const failed = (where: string) => (err: unknown) => {
    logger.error('useLaterActions', where, err);
    toast.error(toUserMessage(err, ErrorLabels.LaterFailed));
  };

  const save = useMutation({
    mutationFn: ({ messageId, remindAt }: { messageId: string; remindAt?: Date }) =>
      api().typed((c) =>
        c.POST('/workspaces/{ws_id}/later', {
          params: { path: { ws_id: ws } },
          body: { message_id: messageId, remind_at: remindAt?.toISOString() ?? null },
        }),
      ),
    onSuccess: refresh,
    onError: failed('save'),
  });

  const remind = useMutation({
    mutationFn: ({ text, remindAt, channelId }: { text: string; remindAt: Date; channelId?: string }) =>
      api().typed((c) =>
        c.POST('/workspaces/{ws_id}/later', {
          params: { path: { ws_id: ws } },
          body: { text, remind_at: remindAt.toISOString(), channel_id: channelId ?? null },
        }),
      ),
    onSuccess: refresh,
    onError: failed('remind'),
  });

  const update = useMutation({
    mutationFn: ({
      id,
      status,
      remindAt,
      clearReminder,
    }: {
      id: string;
      status?: LaterStatus;
      remindAt?: Date;
      clearReminder?: boolean;
    }) =>
      api().typed((c) =>
        c.PATCH('/later/{id}', {
          params: { path: { id } },
          body: {
            status: status ?? null,
            remind_at: remindAt?.toISOString() ?? null,
            clear_reminder: clearReminder ?? false,
          },
        }),
      ),
    onSuccess: refresh,
    onError: failed('update'),
  });

  const remove = useMutation({
    mutationFn: (id: string) => api().typed((c) => c.DELETE('/later/{id}', { params: { path: { id } } })),
    onSuccess: refresh,
    onError: failed('remove'),
  });

  return { save, remind, update, remove };
}
