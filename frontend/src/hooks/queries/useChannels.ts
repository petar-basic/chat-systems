import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useWorkspaceStore } from '@/stores/workspace';
import { instanceManager } from '@/lib/instances';
import { api } from '@/lib/api';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import type { Message, Channel } from '@/stores/workspace';

interface ChannelMember {
  id: string;
  channel_id: string;
  user_id: string;
  role: string;
  joined_at: string;
}

interface ChannelMembersResponse {
  data: ChannelMember[];
}

export interface BrowsableChannel {
  id: string;
  workspace_id: string;
  name: string | null;
  channel_type: string;
  topic: string | null;
  description: string | null;
  is_default: boolean;
  created_at: string;
  member_count: number;
  is_member: boolean;
}

interface PinnedMessagesResponse {
  data: Message[];
}

function useCurrentApi() {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  return instanceUrl ? instanceManager.get(instanceUrl).api : api;
}

export interface ChannelUnreadCount {
  channel_id: string;
  unread_count: number;
  mention_count: number;
}

export const useUnreadChannels = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.channelsUnread(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{
        channel_ids: string[];
        counts: ChannelUnreadCount[];
      }>(`/workspaces/${workspaceId}/channels/unread`);
      return res;
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: 1000 * 30,
  });
};

export const useSetChannelMuted = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ channelId, muted }: { channelId: string; muted: boolean }) =>
      getApiForInstance(instanceUrl).patch(`/channels/${channelId}/notifications`, { muted }),
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
      const response = await getApiForInstance(instanceUrl).get<{ data: BrowsableChannel[] }>(
        `/workspaces/${workspaceId}/channels/browse`,
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
      getApiForInstance(instanceUrl).post(`/channels/${channelId}/join`, {}),
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
      getApiForInstance(instanceUrl).delete(`/channels/${channelId}/members/${userId}`),
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
      const response = await apiClient.get<ChannelMembersResponse>(`/channels/${channelId}/members`);
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
    mutationFn: async ({ userId, role }: { userId: string; role: 'member' | 'admin' }) =>
      apiClient.patch<ChannelMember>(`/channels/${channelId}/members/${userId}/role`, { role }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelMembers(channelId) });
    },
  });
};

export const useUpdateChannel = (workspaceId: string, channelId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: async (patch: {
      name?: string;
      topic?: string;
      description?: string;
      post_policy?: 'everyone' | 'moderators';
    }) =>
      apiClient.patch<Channel>(`/channels/${channelId}`, patch),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
    },
  });
};

export const useArchiveChannel = (workspaceId: string) => {
  const queryClient = useQueryClient();
  const apiClient = useCurrentApi();
  return useMutation({
    mutationFn: async (channelId: string) => apiClient.delete(`/channels/${channelId}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
    },
  });
};

export const useChannelPins = (channelId: string | null) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.channelPins(channelId ?? ''),
    queryFn: async () => {
      if (!channelId) throw new Error('No channel ID');
      const response = await apiClient.get<PinnedMessagesResponse>(`/channels/${channelId}/pins`);
      return response.data;
    },
    enabled: !!channelId,
    staleTime: 1000 * 30,
  });
};
