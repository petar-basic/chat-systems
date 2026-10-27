import { useQuery, useQueries, useMutation, useQueryClient, type QueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi, getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS, NOTIFICATIONS_PAGE_SIZE } from '@/shared/constants';
import { toast } from '@/shared/components/Toast';
import { logger } from '@/lib/logger';
import { ErrorLabels } from '@/shared/constants';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';

export type NotificationKind = components['schemas']['NotificationType'];

export type RawNotification = components['schemas']['Notification'];

export type DndStatus = components['schemas']['DndStatus'];

export interface AppNotification {
  id: string;
  workspace_id: string;
  user_id: string;
  notification_type: NotificationKind;
  title: string;
  body: string;
  channel_id: string | null;
  message_id: string | null;
  thread_parent_id: string | null;
  list_id: string | null;
  item_id: string | null;
  is_read: boolean;
  created_at: string;
}

type DeepLink = Pick<
  AppNotification,
  'channel_id' | 'message_id' | 'thread_parent_id' | 'list_id' | 'item_id'
>;

function deepLink(data: unknown): DeepLink {
  const fields: Record<string, unknown> = typeof data === 'object' && data !== null ? { ...data } : {};
  const text = (key: keyof DeepLink) => {
    const value = fields[key];
    return typeof value === 'string' ? value : null;
  };
  return {
    channel_id: text('channel_id'),
    message_id: text('message_id'),
    thread_parent_id: text('thread_parent_id'),
    list_id: text('list_id'),
    item_id: text('item_id'),
  };
}

export function normalizeNotification(raw: RawNotification): AppNotification {
  return {
    id: raw.id,
    workspace_id: raw.workspace_id,
    user_id: raw.user_id,
    notification_type: raw.notification_type,
    title: raw.title,
    body: raw.body ?? '',
    ...deepLink(raw.data),
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
      const res = await apiClient.typed((c) =>
        c.GET('/workspaces/{ws_id}/notifications', {
          params: { path: { ws_id: workspaceId }, query: { limit: NOTIFICATIONS_PAGE_SIZE } },
        }),
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
      const res = await apiClient.typed((c) =>
        c.GET('/workspaces/{ws_id}/notifications/unread-count', { params: { path: { ws_id: workspaceId } } }),
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
        const res = await getApiForInstance(ws.instanceUrl).typed((c) =>
          c.GET('/workspaces/{ws_id}/notifications/unread-count', { params: { path: { ws_id: ws.id } } }),
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
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const workspaceId = workspace?.id ?? '';
  return useQuery({
    queryKey: QUERY_KEYS.notificationDnd(workspaceId),
    queryFn: async () =>
      getApiForInstance(workspace?.instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/notifications/dnd', { params: { path: { ws_id: workspaceId } } }),
      ),
    enabled: !!workspace,
    staleTime: 1000 * 60,
  });
};

export const useSetDnd = () => {
  const queryClient = useQueryClient();
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const workspaceId = workspace?.id ?? '';
  const key = QUERY_KEYS.notificationDnd(workspaceId);
  return useMutation({
    mutationFn: async (dndUntil: string | null) =>
      getApiForInstance(workspace?.instanceUrl).typed((c) =>
        c.PATCH('/workspaces/{ws_id}/notifications/dnd', {
          params: { path: { ws_id: workspaceId } },
          body: { dnd_until: dndUntil },
        }),
      ),
    onMutate: (dndUntil) => {
      const previous = queryClient.getQueryData<DndStatus>(key);
      queryClient.setQueryData<DndStatus>(key, {
        dnd_until: dndUntil,
        schedule_paused_until: dndUntil ? (previous?.schedule_paused_until ?? null) : null,
      });
      return { previous };
    },
    onSuccess: (status) => {
      queryClient.setQueryData(key, status);
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });
    },
    onError: (err, _vars, ctx) => {
      logger.error('useSetDnd', 'mutationFn', err);
      if (ctx) queryClient.setQueryData(key, ctx.previous);
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
      return apiClient.typed((c) =>
        c.POST('/notifications/read', { body: { notification_ids: notificationIds } }),
      );
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

export async function settleChannelNotifications(
  queryClient: QueryClient,
  workspaceId: string,
  channelId: string,
  unread: number,
) {
  const countKey = QUERY_KEYS.notificationUnreadCount(workspaceId);
  await queryClient.cancelQueries({ queryKey: countKey, exact: true });
  queryClient.setQueryData<number>(countKey, unread);
  queryClient.setQueryData<AppNotification[]>(QUERY_KEYS.notifications(workspaceId), (old) =>
    old?.map((n) =>
      n.channel_id === channelId && !n.thread_parent_id && !n.is_read ? { ...n, is_read: true } : n,
    ),
  );
}

export const useMarkAllNotificationsRead = (workspaceId: string | null) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  const listKey = QUERY_KEYS.notifications(workspaceId ?? '');
  const countKey = QUERY_KEYS.notificationUnreadCount(workspaceId ?? '');

  return useMutation({
    mutationFn: async () => {
      if (!workspaceId) throw new Error('No workspace');
      return apiClient.typed((c) =>
        c.POST('/workspaces/{ws_id}/notifications/read-all', { params: { path: { ws_id: workspaceId } } }),
      );
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
