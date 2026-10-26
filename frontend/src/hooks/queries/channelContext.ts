import type { QueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import type { ApiClient } from '@/lib/api';
import { QUERY_KEYS } from '@/shared/constants';

export type ChannelContext = components['schemas']['ChannelContext'];

const FRESH_FOR = 1000 * 30;

function fetchChannelContext(queryClient: QueryClient, api: ApiClient, channelId: string) {
  return queryClient.fetchQuery({
    queryKey: QUERY_KEYS.channelContext(channelId),
    queryFn: async (): Promise<ChannelContext> => {
      const context = await api.typed((c) =>
        c.GET('/channels/{ch_id}/context', { params: { path: { ch_id: channelId } } }),
      );
      queryClient.setQueryData(QUERY_KEYS.channelMembers(channelId), context.members);
      queryClient.setQueryData(QUERY_KEYS.channelPins(channelId), context.pins);
      queryClient.setQueryData(QUERY_KEYS.channelBookmarks(channelId), context.bookmarks);
      queryClient.setQueryData(QUERY_KEYS.channelLists(context.workspace_id, channelId), context.lists);
      queryClient.setQueryData(QUERY_KEYS.workflowShortcuts(channelId), context.workflow_shortcuts);
      return context;
    },
  });
}

export async function fromChannelContext<T>(
  queryClient: QueryClient,
  api: ApiClient,
  channelId: string,
  pick: (context: ChannelContext) => T,
  own: () => Promise<T>,
): Promise<T> {
  const state = queryClient.getQueryState(QUERY_KEYS.channelContext(channelId));
  if (state?.fetchStatus === 'idle' && state.dataUpdatedAt > Date.now() - FRESH_FOR) return own();
  return pick(await fetchChannelContext(queryClient, api, channelId));
}
