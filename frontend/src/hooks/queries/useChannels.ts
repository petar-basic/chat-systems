import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useWorkspaceStore } from '@/stores/workspace';
import { useCurrentApi, getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import type { Message, Channel } from '@/stores/workspace';

export type BrowsableChannel = components['schemas']['BrowsableChannel'];

export const useUnreadChannels = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.channelsUnread(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace ID');
      return getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/channels/unread', { params: { path: { ws_id: workspaceId } } }),
      );
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: 1000 * 30,
  });
};

export const useSetChannelMuted = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ channelId, muted }: { channelId: string; muted: boolean }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PATCH('/channels/{ch_id}/notifications', {
          params: { path: { ch_id: channelId } },
          body: { muted },
        }),
      ),
    onMutate: ({ channelId, muted }) => {
      useWorkspaceStore.getState().setChannelMuted(channelId, muted);
      queryClient.setQueryData<Channel[]>(QUERY_KEYS.workspaceChannels(workspaceId), (old) =>
        old?.map((c) => (c.id === channelId ? { ...c, muted } : c)),
      );
    },
  });
};

export const useBrowsableChannels = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.channelsBrowse(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace ID');
      const response = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/channels/browse', { params: { path: { ws_id: workspaceId } } }),
      );
      return response.data;
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 30,
  });
};

export const useJoinChannel = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (channelId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/channels/{ch_id}/join', { params: { path: { ch_id: channelId } } }),
      ),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelsBrowse(workspaceId) }),
      ]);
    },
  });
};

export const useLeaveChannel = (workspaceId: string, userId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (channelId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/channels/{ch_id}/members/{user_id}', {
          params: { path: { ch_id: channelId, user_id: userId } },
        }),
      ),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelsBrowse(workspaceId) }),
      ]);
    },
  });
};

export const useChannelMembers = (channelId: string | null) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.channelMembers(channelId ?? ''),
    queryFn: async () => {
      if (!channelId) throw new Error('No channel ID');
      const response = await apiClient.typed((c) =>
        c.GET('/channels/{ch_id}/members', { params: { path: { ch_id: channelId } } }),
      );
      return response.data;
    },
    enabled: !!channelId,
    staleTime: 1000 * 60 * 2,
  });
};

export const useUpdateChannelMemberRole = (channelId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: async ({ userId, role }: { userId: string; role: components['schemas']['ChannelRole'] }) =>
      apiClient.typed((c) =>
        c.PATCH('/channels/{ch_id}/members/{user_id}/role', {
          params: { path: { ch_id: channelId, user_id: userId } },
          body: { role },
        }),
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelMembers(channelId) });
    },
  });
};

export const useUpdateChannel = (workspaceId: string, channelId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: async (patch: components['schemas']['UpdateChannelRequest']) =>
      apiClient.typed((c) =>
        c.PATCH('/channels/{ch_id}', { params: { path: { ch_id: channelId } }, body: patch }),
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
    },
  });
};

export const useArchiveChannel = (workspaceId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: async (channelId: string) =>
      apiClient.typed((c) => c.DELETE('/channels/{ch_id}', { params: { path: { ch_id: channelId } } })),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
    },
  });
};

export const useChannelPins = (channelId: string | null) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.channelPins(channelId ?? ''),
    queryFn: async (): Promise<Message[]> => {
      if (!channelId) throw new Error('No channel ID');
      const response = await apiClient.typed((c) =>
        c.GET('/channels/{ch_id}/pins', { params: { path: { ch_id: channelId } } }),
      );
      return response.data;
    },
    enabled: !!channelId,
    staleTime: 1000 * 30,
  });
};
