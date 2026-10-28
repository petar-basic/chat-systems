import { useInfiniteQuery, useMutation, useQueryClient, type InfiniteData } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { applyReadState } from '@/lib/readState';
import { QUERY_KEYS, ErrorLabels, ActionLabels } from '@/shared/constants';
import { useWorkspaceStore } from '@/stores/workspace';
import { toast } from '@/shared/components/Toast';
import { logger } from '@/lib/logger';

export type UnreadGroup = components['schemas']['UnreadGroup'];
type UnreadsPage = components['schemas']['UnreadsPage'];

const CHANNELS_PER_PAGE = 20;

export function useUnreads(workspaceId: string | undefined) {
  const apiClient = useCurrentApi();
  return useInfiniteQuery({
    queryKey: QUERY_KEYS.unreads(workspaceId ?? ''),
    enabled: !!workspaceId,
    initialPageParam: 0,
    queryFn: ({ pageParam }): Promise<UnreadsPage> =>
      apiClient.typed((c) =>
        c.GET('/workspaces/{ws_id}/unreads', {
          params: {
            path: { ws_id: workspaceId ?? '' },
            query: { limit: CHANNELS_PER_PAGE, offset: pageParam },
          },
        }),
      ),
    getNextPageParam: (page) => page.next_offset ?? undefined,
  });
}

export function useMarkGroupRead(workspaceId: string | undefined) {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: async (group: UnreadGroup) => {
      const newest = group.messages[group.messages.length - 1];
      if (!newest) return;
      await apiClient.typed((c) =>
        c.POST('/channels/{ch_id}/read', {
          params: { path: { ch_id: group.channel_id } },
          body: { message_id: newest.id },
        }),
      );
    },
    onSuccess: (_data, group) => {
      applyReadState(queryClient, { channel_id: group.channel_id, unread_count: 0, mention_count: 0 });
      queryClient.setQueryData<InfiniteData<UnreadsPage>>(QUERY_KEYS.unreads(workspaceId ?? ''), (old) =>
        old
          ? {
              ...old,
              pages: old.pages.map((page) => ({
                ...page,
                data: page.data.filter((g) => g.channel_id !== group.channel_id),
              })),
            }
          : old,
      );
    },
    onError: (err) => {
      logger.error('useMarkGroupRead', 'mutationFn', err);
      toast.error(ErrorLabels.MarkReadFailed);
    },
  });
}

export function useMarkWorkspaceRead(workspaceId: string | undefined) {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: () =>
      apiClient.typed((c) =>
        c.POST('/workspaces/{ws_id}/read', { params: { path: { ws_id: workspaceId ?? '' } } }),
      ),
    onSuccess: () => {
      useWorkspaceStore.getState().markEverythingRead();
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.unreads(workspaceId ?? '') });
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelsUnread(workspaceId ?? '') });
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.conversations(workspaceId ?? '') });
      toast.success(ActionLabels.MarkedAllRead);
    },
    onError: (err) => {
      logger.error('useMarkWorkspaceRead', 'mutationFn', err);
      toast.error(ErrorLabels.MarkAllReadFailed);
    },
  });
}
