import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export const useChannelBookmarks = (channelId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.channelBookmarks(channelId ?? ''),
    queryFn: async () => {
      if (!channelId) throw new Error('No channel selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/channels/{ch_id}/bookmarks', { params: { path: { ch_id: channelId } } }),
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
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/channels/{ch_id}/bookmarks', {
          params: { path: { ch_id: channelId } },
          body: { label, url, emoji: emoji || null },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelBookmarks(channelId) });
    },
  });
};

export const useDeleteChannelBookmark = (channelId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (bookmarkId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/channels/{ch_id}/bookmarks/{bookmark_id}', {
          params: { path: { ch_id: channelId, bookmark_id: bookmarkId } },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelBookmarks(channelId) });
    },
  });
};
