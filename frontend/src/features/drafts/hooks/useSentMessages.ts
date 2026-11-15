import { useInfiniteQuery } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

const PAGE_SIZE = 30;

export function useSentMessages(workspaceId: string, instanceUrl?: string) {
  return useInfiniteQuery({
    queryKey: QUERY_KEYS.sentMessages(workspaceId),
    queryFn: ({ pageParam }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/sent-messages', {
          params: { path: { ws_id: workspaceId }, query: { cursor: pageParam, limit: PAGE_SIZE } },
        }),
      ),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) =>
      page.data.length < PAGE_SIZE ? undefined : page.data[page.data.length - 1]?.id,
    staleTime: 0,
  });
}
