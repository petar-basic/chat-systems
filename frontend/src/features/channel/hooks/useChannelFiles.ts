import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type ChannelFile = components['schemas']['DataList_ChannelFile']['data'][number];
export type FileKindFilter = 'image' | 'video' | 'audio' | 'document';

const PAGE_SIZE = 50;
const NAME_MATCHES = 100;

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

export function useChannelFileSearch(workspaceId: string | undefined, channelId: string, name: string) {
  const apiClient = useCurrentApi();
  return useQuery({
    queryKey: QUERY_KEYS.channelFileSearch(channelId, name),
    queryFn: () =>
      apiClient.typed((c) =>
        c.GET('/search/files', {
          params: {
            query: { workspace_id: workspaceId ?? '', channel_id: channelId, q: name, limit: NAME_MATCHES },
          },
        }),
      ),
    select: (page) => page.data,
    enabled: !!workspaceId && name.length > 0,
  });
}
