import { useInfiniteQuery } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type ChannelFile = components['schemas']['DataList_ChannelFile']['data'][number];
export type FileKindFilter = 'image' | 'video' | 'audio' | 'document';

const PAGE_SIZE = 50;

export function useChannelFiles(channelId: string, kind: FileKindFilter | null) {
  const apiClient = useCurrentApi();
  return useInfiniteQuery({
    queryKey: QUERY_KEYS.channelFiles(channelId, kind ?? 'all'),
    queryFn: ({ pageParam }) =>
      apiClient.typed((c) =>
        c.GET('/channels/{ch_id}/files', {
          params: {
            path: { ch_id: channelId },
            query: { kind: kind ?? undefined, cursor: pageParam, limit: PAGE_SIZE },
          },
        }),
      ),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) =>
      page.data.length < PAGE_SIZE ? undefined : page.data[page.data.length - 1]?.id,
    staleTime: 0,
  });
}
