import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import type { Channel } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging/hooks/useConversations';

export type NotifyLevel = components['schemas']['NotifyLevel'];

export const useNotifyDefault = (workspaceId: string | null, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.notifyDefault(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace ID');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/notify-default', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.notify_default;
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: 1000 * 60 * 5,
  });

export const useSetNotifyDefault = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (level: NotifyLevel) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PUT('/workspaces/{ws_id}/notify-default', {
          params: { path: { ws_id: workspaceId } },
          body: { level },
        }),
      ),
    onMutate: (level) => {
      const previous = queryClient.getQueryData<NotifyLevel>(QUERY_KEYS.notifyDefault(workspaceId));
      queryClient.setQueryData(QUERY_KEYS.notifyDefault(workspaceId), level);
      return { previous };
    },
    onError: (_err, _level, context) => {
      queryClient.setQueryData(QUERY_KEYS.notifyDefault(workspaceId), context?.previous);
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.conversations(workspaceId) });
    },
  });
};

export const useSetNotifyLevel = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  const channelsKey = QUERY_KEYS.workspaceChannels(workspaceId);
  const conversationsKey = QUERY_KEYS.conversations(workspaceId);
  return useMutation({
    mutationFn: ({ channelId, level }: { channelId: string; level: NotifyLevel | null }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PUT('/channels/{ch_id}/notify-level', {
          params: { path: { ch_id: channelId } },
          body: { level },
        }),
      ),
    onSuccess: (saved, { channelId }) => {
      queryClient.setQueryData<Channel[]>(channelsKey, (old) =>
        old?.map((c) => (c.id === channelId ? { ...c, ...saved } : c)),
      );
      queryClient.setQueryData<Conversation[]>(conversationsKey, (old) =>
        old?.map((c) => (c.id === channelId ? { ...c, ...saved } : c)),
      );
    },
  });
};
