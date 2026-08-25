import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export interface ChannelBookmark {
  id: string;
  channel_id: string;
  created_by: string | null;
  label: string;
  url: string;
  emoji: string | null;
  position: number;
  created_at: string;
}

export const useChannelBookmarks = (channelId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.channelBookmarks(channelId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: ChannelBookmark[] }>(
        `/channels/${channelId}/bookmarks`,
      );
      return res.data;
    },
    enabled: !!channelId && !!instanceUrl,
    staleTime: 1000 * 60 * 5,
  });
};

export const useCreateChannelBookmark = (channelId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ label, url, emoji }: { label: string; url: string; emoji?: string }) =>
      getApiForInstance(instanceUrl).post<ChannelBookmark>(`/channels/${channelId}/bookmarks`, {
        label,
        url,
        emoji: emoji || null,
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelBookmarks(channelId) });
    },
  });
};

export const useDeleteChannelBookmark = (channelId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (bookmarkId: string) =>
      getApiForInstance(instanceUrl).delete(`/channels/${channelId}/bookmarks/${bookmarkId}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelBookmarks(channelId) });
    },
  });
};
