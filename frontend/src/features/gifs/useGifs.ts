import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { ApiError } from '@/lib/errors';
import { findGifs } from './giphy';
import type { GifSettings } from './types';

export function useGifSettings(workspaceId: string | undefined, instanceUrl?: string) {
  return useQuery({
    queryKey: QUERY_KEYS.gifSettings(workspaceId ?? ''),
    enabled: !!workspaceId,
    staleTime: 60_000,
    retry: false,
    queryFn: async (): Promise<GifSettings | null> => {
      if (!workspaceId) return null;
      try {
        return await getApiForInstance(instanceUrl).typed((c) =>
          c.GET('/workspaces/{ws_id}/gifs', { params: { path: { ws_id: workspaceId } } }),
        );
      } catch (error) {
        if (error instanceof ApiError && error.status === 404) return null;
        throw error;
      }
    },
  });
}

export function useGifSearch(settings: GifSettings, query: string) {
  return useInfiniteQuery({
    queryKey: QUERY_KEYS.gifSearch(settings.rating, query.trim()),
    queryFn: ({ pageParam, signal }) => findGifs(settings, query, pageParam, signal),
    initialPageParam: 0,
    getNextPageParam: (last) => last.next,
    staleTime: 0,
    gcTime: 0,
  });
}
