import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';
import { useChannelMembers, useChannelPins } from './useChannels';
import { useChannelBookmarks } from './useChannelBookmarks';
import { useChannelLists } from '@/features/lists/useLists';
import { useWorkflowShortcuts } from '@/features/workflows/useWorkflows';

const get = vi.fn();
const client = {
  typed: async (call: (c: { GET: typeof get }) => Promise<{ data: unknown }>) =>
    (await call({ GET: get })).data,
};

vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => client,
  useCurrentApi: () => client,
}));

const context = {
  workspace_id: 'ws-1',
  members: [{ channel_id: 'ch-1', user_id: 'u-1', role: 'admin' }],
  pins: [{ id: 'm-1', content: 'the plan' }],
  bookmarks: [{ id: 'b-1', label: 'Runbook' }],
  lists: [{ id: 'l-1', title: 'Launch' }],
  workflow_shortcuts: [{ id: 'w-1', name: 'Ask' }],
};

function setup() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const view = renderHook(
    () => ({
      members: useChannelMembers('ch-1').data,
      pins: useChannelPins('ch-1').data,
      bookmarks: useChannelBookmarks('ch-1', 'http://chat.test').data,
      lists: useChannelLists('ws-1', 'ch-1', 'http://chat.test').data,
      shortcuts: useWorkflowShortcuts('ch-1', 'http://chat.test').data,
    }),
    { wrapper },
  );
  return { queryClient, view };
}

describe('channel context', () => {
  beforeEach(() => {
    get.mockReset();
    get.mockImplementation(async (path: string) =>
      path === '/channels/{ch_id}/context' ? { data: context } : { data: { data: [{ id: 'own' }] } },
    );
  });

  it('answers every piece of the channel header with one request', async () => {
    const { view } = setup();

    await waitFor(() => expect(view.result.current.shortcuts).toBeDefined());
    expect(view.result.current).toEqual({
      members: context.members,
      pins: context.pins,
      bookmarks: context.bookmarks,
      lists: context.lists,
      shortcuts: context.workflow_shortcuts,
    });
    expect(get).toHaveBeenCalledTimes(1);
    expect(get).toHaveBeenCalledWith('/channels/{ch_id}/context', { params: { path: { ch_id: 'ch-1' } } });
  });

  it('seeds the keys the components read', async () => {
    const { queryClient, view } = setup();
    await waitFor(() => expect(view.result.current.shortcuts).toBeDefined());

    expect(queryClient.getQueryData(QUERY_KEYS.channelMembers('ch-1'))).toEqual(context.members);
    expect(queryClient.getQueryData(QUERY_KEYS.channelPins('ch-1'))).toEqual(context.pins);
    expect(queryClient.getQueryData(QUERY_KEYS.channelBookmarks('ch-1'))).toEqual(context.bookmarks);
    expect(queryClient.getQueryData(QUERY_KEYS.channelLists('ws-1', 'ch-1'))).toEqual(context.lists);
    expect(queryClient.getQueryData(QUERY_KEYS.workflowShortcuts('ch-1'))).toEqual(
      context.workflow_shortcuts,
    );
  });

  it('refetches only the invalidated key from its own route', async () => {
    const { queryClient, view } = setup();
    await waitFor(() => expect(view.result.current.shortcuts).toBeDefined());

    await act(() => queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelPins('ch-1') }));

    await waitFor(() => expect(view.result.current.pins).toEqual([{ id: 'own' }]));
    expect(get).toHaveBeenCalledTimes(2);
    expect(get).toHaveBeenLastCalledWith('/channels/{ch_id}/pins', { params: { path: { ch_id: 'ch-1' } } });
    expect(view.result.current.members).toEqual(context.members);
  });
});
