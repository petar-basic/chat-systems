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

interface PinnedMessagesResponse {
  data: Message[];
}

function useCurrentApi() {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  return instanceUrl ? instanceManager.get(instanceUrl).api : api;
}

export const useUnreadChannelIds = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.channelsUnread(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ channel_ids: string[] }>(
        `/workspaces/${workspaceId}/channels/unread`,
      );
      return res.channel_ids;
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
