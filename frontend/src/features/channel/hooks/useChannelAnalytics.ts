import { useQuery } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type ChannelAnalytics = components['schemas']['ChannelAnalytics'];

export const useChannelAnalytics = (channelId: string) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.channelAnalytics(channelId),
    queryFn: () =>
      apiClient.typed((c) =>
        c.GET('/channels/{ch_id}/analytics', { params: { path: { ch_id: channelId } } }),
      ),
  });
};
