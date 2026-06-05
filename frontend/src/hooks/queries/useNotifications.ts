import { useQuery, useQueries, useMutation, useQueryClient } from '@tanstack/react-query';
import { useCurrentApi, getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS, NOTIFICATIONS_PAGE_SIZE } from '@/shared/constants';
import { toast } from '@/shared/components/Toast';
import { logger } from '@/lib/logger';
import { ErrorLabels } from '@/shared/constants';
import type { Workspace } from '@/stores/workspace';

export type NotificationKind = 'mention' | 'dm' | 'reply' | 'reaction' | 'call' | 'reminder' | 'system';

export interface AppNotification {
  id: string;
  workspace_id: string;
  user_id: string;
  notification_type: NotificationKind;
  title: string;
  body: string;
  channel_id: string | null;
  message_id: string | null;
  is_read: boolean;
  created_at: string;
}

export interface RawNotification {
  id: string;
  workspace_id: string;
  user_id: string;
  notification_type: NotificationKind;
  title: string;
  body: string | null;
  data: { channel_id?: string | null; message_id?: string | null } | null;
  is_read: boolean;
  created_at: string;
}

interface NotificationsResponse {
  data: RawNotification[];
}

interface UnreadCountResponse {
  unread_count: number;
}

export function normalizeNotification(raw: RawNotification): AppNotification {
  return {
    id: raw.id,
    workspace_id: raw.workspace_id,
    user_id: raw.user_id,
    notification_type: raw.notification_type,
    title: raw.title,
    body: raw.body ?? '',
    channel_id: raw.data?.channel_id ?? null,
    message_id: raw.data?.message_id ?? null,
    is_read: raw.is_read,
    created_at: raw.created_at,
  };
}

export const useNotifications = (workspaceId: string | null) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.notifications(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace');
      const res = await apiClient.get<NotificationsResponse>(
        `/workspaces/${workspaceId}/notifications?limit=${NOTIFICATIONS_PAGE_SIZE}`,
      );
      return res.data.map(normalizeNotification);
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 30,
  });
};

export const useUnreadNotificationCount = (workspaceId: string | null) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.notificationUnreadCount(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace');
      const res = await apiClient.get<UnreadCountResponse>(
        `/workspaces/${workspaceId}/notifications/unread-count`,
      );
      return res.unread_count;
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 30,
  });
};

export const useWorkspaceUnreadCounts = (workspaces: Workspace[]): Record<string, number> => {
  const results = useQueries({
    queries: workspaces.map((ws) => ({
      queryKey: QUERY_KEYS.notificationUnreadCount(ws.id),
      queryFn: async () => {
        const res = await getApiForInstance(ws.instanceUrl).get<UnreadCountResponse>(
          `/workspaces/${ws.id}/notifications/unread-count`,
        );
        return res.unread_count;
      },
      staleTime: 1000 * 30,
    })),
  });

  const counts: Record<string, number> = {};
  workspaces.forEach((ws, i) => {
    counts[ws.id] = results[i]?.data ?? 0;
  });
  return counts;
};

export const useDndStatus = () => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.notificationDnd(),
    queryFn: async () => {
      const res = await apiClient.get<{ dnd_until: string | null }>('/notifications/dnd');
      return res.dnd_until;
    },
    staleTime: 1000 * 60,
  });
};

export const useSetDnd = () => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: async (dndUntil: string | null) =>
      apiClient.patch('/notifications/dnd', { dnd_until: dndUntil }),
    onMutate: (dndUntil) => {
      const previous = queryClient.getQueryData<string | null>(QUERY_KEYS.notificationDnd());
      queryClient.setQueryData(QUERY_KEYS.notificationDnd(), dndUntil);
      return { previous };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useSetDnd', 'mutationFn', err);
      if (ctx) queryClient.setQueryData(QUERY_KEYS.notificationDnd(), ctx.previous ?? null);
      toast.error(ErrorLabels.NotFound);
    },
  });
};

export const useMarkNotificationsRead = (workspaceId: string | null) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  const listKey = QUERY_KEYS.notifications(workspaceId ?? '');
  const countKey = QUERY_KEYS.notificationUnreadCount(workspaceId ?? '');

  return useMutation({
    mutationFn: async (notificationIds: string[]) => {
      return apiClient.post('/notifications/read', { notification_ids: notificationIds });
    },
    onMutate: async (notificationIds) => {
      await queryClient.cancelQueries({ queryKey: listKey });
      const prevList = queryClient.getQueryData<AppNotification[]>(listKey);
      const prevCount = queryClient.getQueryData<number>(countKey);
      const ids = new Set(notificationIds);
      let newlyRead = 0;
      queryClient.setQueryData<AppNotification[]>(listKey, (old) =>
        old?.map((n) => {
          if (ids.has(n.id) && !n.is_read) {
            newlyRead++;
            return { ...n, is_read: true };
          }
          return n;
        }),
      );
      queryClient.setQueryData<number>(countKey, (c) => Math.max(0, (c ?? 0) - newlyRead));
      return { prevList, prevCount };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useMarkNotificationsRead', 'mutationFn', err);
      if (ctx?.prevList) queryClient.setQueryData(listKey, ctx.prevList);
      if (ctx?.prevCount !== undefined) queryClient.setQueryData(countKey, ctx.prevCount);
      toast.error(ErrorLabels.NotFound);
    },
  });
};

export const useMarkChannelNotificationsRead = (workspaceId: string | null) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  const listKey = QUERY_KEYS.notifications(workspaceId ?? '');
  const countKey = QUERY_KEYS.notificationUnreadCount(workspaceId ?? '');

  return useMutation({
    mutationFn: async (channelId: string) => {
      if (!workspaceId) throw new Error('No workspace');
      return apiClient.post(`/workspaces/${workspaceId}/channels/${channelId}/notifications/read`, {});
    },
    onMutate: async (channelId) => {
      await queryClient.cancelQueries({ queryKey: listKey });
      const prevList = queryClient.getQueryData<AppNotification[]>(listKey);
      const prevCount = queryClient.getQueryData<number>(countKey);
      let newlyRead = 0;
      queryClient.setQueryData<AppNotification[]>(listKey, (old) =>
        old?.map((n) => {
          if (n.channel_id === channelId && !n.is_read) {
            newlyRead++;
            return { ...n, is_read: true };
          }
          return n;
        }),
      );
      if (newlyRead > 0) {
        queryClient.setQueryData<number>(countKey, (c) => Math.max(0, (c ?? 0) - newlyRead));
      }
      return { prevList, prevCount };
    },
    onError: (err, _channelId, ctx) => {
      logger.error('useMarkChannelNotificationsRead', 'mutationFn', err);
      if (ctx?.prevList) queryClient.setQueryData(listKey, ctx.prevList);
      if (ctx?.prevCount !== undefined) queryClient.setQueryData(countKey, ctx.prevCount);
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: countKey });
    },
  });
};

export const useMarkAllNotificationsRead = (workspaceId: string | null) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  const listKey = QUERY_KEYS.notifications(workspaceId ?? '');
  const countKey = QUERY_KEYS.notificationUnreadCount(workspaceId ?? '');

  return useMutation({
    mutationFn: async () => {
      if (!workspaceId) throw new Error('No workspace');
      return apiClient.post(`/workspaces/${workspaceId}/notifications/read-all`, {});
    },
    onMutate: async () => {
      await queryClient.cancelQueries({ queryKey: listKey });
      const prevList = queryClient.getQueryData<AppNotification[]>(listKey);
      const prevCount = queryClient.getQueryData<number>(countKey);
      queryClient.setQueryData<AppNotification[]>(listKey, (old) =>
        old?.map((n) => ({ ...n, is_read: true })),
      );
      queryClient.setQueryData<number>(countKey, 0);
      return { prevList, prevCount };
    },
    onError: (err, _vars, ctx) => {
      logger.error('useMarkAllNotificationsRead', 'mutationFn', err);
      if (ctx?.prevList) queryClient.setQueryData(listKey, ctx.prevList);
      if (ctx?.prevCount !== undefined) queryClient.setQueryData(countKey, ctx.prevCount);
    },
  });
};
