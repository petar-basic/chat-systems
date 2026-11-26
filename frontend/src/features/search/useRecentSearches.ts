import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import { logger } from '@/lib/logger';
import i18n from '@/lib/i18n';
import { forTab, hasFilters, type SearchFilters, type SearchTab } from './searchFilters';

export type RecentSearch = components['schemas']['RecentSearch'];

export interface SearchToKeep {
  query: string;
  tab: SearchTab;
  filters: SearchFilters;
}

export function filtersOf(search: RecentSearch): SearchFilters {
  const kept: SearchFilters = {};
  const saved = search.filters;
  if (saved.from) kept.from = saved.from;
  if (saved.in) kept.in = saved.in;
  if (saved.has) kept.has = saved.has;
  if (saved.kind) kept.kind = saved.kind;
  if (saved.on) kept.on = saved.on;
  if (saved.after) kept.after = saved.after;
  if (saved.before) kept.before = saved.before;
  return kept;
}

export function useRecentSearches(workspaceId: string | undefined, instanceUrl?: string) {
  return useQuery({
    queryKey: QUERY_KEYS.recentSearches(workspaceId ?? ''),
    enabled: !!workspaceId,
    staleTime: 0,
    queryFn: () =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/searches/recent', { params: { path: { ws_id: workspaceId ?? '' } } }),
      ),
    select: (res) => res.data,
  });
}

export function useRecentSearchActions(workspaceId: string | undefined, instanceUrl?: string) {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.recentSearches(workspaceId ?? '');
  const path = { params: { path: { ws_id: workspaceId ?? '' } } };
  const api = () => getApiForInstance(instanceUrl);
  const refresh = () => void queryClient.invalidateQueries({ queryKey: key });
  const keep = (update: (searches: RecentSearch[]) => RecentSearch[]) =>
    queryClient.setQueryData<{ data: RecentSearch[] }>(key, (old) =>
      old ? { data: update(old.data) } : old,
    );
  const failed = (where: string) => (err: unknown) => {
    logger.error('useRecentSearchActions', where, err);
    toast.error(toUserMessage(err, i18n.t('search:recent.changeFailed')));
    refresh();
  };

  const record = useMutation({
    mutationFn: ({ query, tab, filters }: SearchToKeep) =>
      api().typed((c) =>
        c.POST('/workspaces/{ws_id}/searches/recent', {
          ...path,
          body: { query, tab, filters: forTab(filters, tab) },
        }),
      ),
    onSuccess: refresh,
    onError: (err) => logger.error('useRecentSearchActions', 'record', err),
  });

  const forget = useMutation({
    mutationFn: (id: string) =>
      api().typed((c) =>
        c.DELETE('/workspaces/{ws_id}/searches/recent/{id}', {
          params: { path: { ws_id: workspaceId ?? '', id } },
        }),
      ),
    onMutate: (id) => keep((searches) => searches.filter((search) => search.id !== id)),
    onError: failed('forget'),
  });

  const clear = useMutation({
    mutationFn: () => api().typed((c) => c.DELETE('/workspaces/{ws_id}/searches/recent', path)),
    onMutate: () => keep(() => []),
    onError: failed('clear'),
  });

  return {
    record: (search: SearchToKeep) => {
      if (!workspaceId || (!search.query.trim() && !hasFilters(forTab(search.filters, search.tab)))) return;
      record.mutate(search);
    },
    forget: (id: string) => forget.mutate(id),
    clear: () => clear.mutate(),
  };
}
