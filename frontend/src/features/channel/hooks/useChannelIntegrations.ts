import { useQuery } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type ChannelIntegrations = components['schemas']['ChannelIntegrations'];

export const useChannelIntegrations = (channelId: string) => {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.channelIntegrations(channelId),
    queryFn: () =>
      apiClient.typed((c) =>
        c.GET('/channels/{ch_id}/integrations', { params: { path: { ch_id: channelId } } }),
      ),
  });
};
