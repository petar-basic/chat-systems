import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore } from '@/stores/workspace';
import { newestSentIn, readOnScreen } from './readOnScreen';
import { belongsElsewhere } from './channelWorkspace';

const post = vi.fn();
vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({
    typed: async (call: (c: { POST: typeof post }) => Promise<unknown>) => call({ POST: post }),
  }),
}));

const message = (id: string, at: string, extra = {}) => ({
  id,
  channel_id: 'ch-1',
  created_at: at,
  ...extra,
});

function seeded() {
  const queryClient = new QueryClient();
  queryClient.setQueryData(QUERY_KEYS.messages('ch-1'), {
    pages: [
      { data: [message('m3', '2026-10-27T10:03:00Z'), message('m2', '2026-10-27T10:02:00Z')] },
      { data: [message('m1', '2026-10-27T10:01:00Z')] },
    ],
    pageParams: [undefined, 'm2'],
  });
  return queryClient;
}

describe('readOnScreen', () => {
  let focused = true;
  beforeEach(() => {
    vi.useFakeTimers();
    post.mockReset();
    post.mockResolvedValue({});
    focused = true;
    vi.spyOn(document, 'hasFocus').mockImplementation(() => focused);
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1' } as never,
      currentChannel: { id: 'ch-1' } as never,
      currentConversationId: null,
    });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('finds the newest message that reached the server, whatever page holds it', () => {
    const queryClient = seeded();
    queryClient.setQueryData(QUERY_KEYS.messages('ch-1'), (old: { pages: { data: unknown[] }[] }) => ({
      ...old,
      pages: [
        { data: [message('local', '2026-10-27T10:09:00Z', { pending: true }), ...old.pages[0].data] },
        ...old.pages.slice(1),
      ],
    }));
    expect(newestSentIn(queryClient, 'ch-1')).toBe('m3');
  });

  it('reads the open channel up to its newest message, once for a burst', async () => {
    const queryClient = seeded();
    readOnScreen(queryClient, 'ch-1');
    readOnScreen(queryClient, 'ch-1');
    await vi.runAllTimersAsync();

    expect(post).toHaveBeenCalledTimes(1);
    expect(post).toHaveBeenCalledWith('/channels/{ch_id}/read', {
      params: { path: { ch_id: 'ch-1' } },
      body: { message_id: 'm3' },
    });
  });

  it('reads the open conversation up to its newest message, as a channel', async () => {
    useWorkspaceStore.setState({ currentChannel: null, currentConversationId: 'dm-1' });
    const queryClient = new QueryClient();
    queryClient.setQueryData(QUERY_KEYS.messages('dm-1'), {
      pages: [{ data: [message('d1', '2026-10-27T10:01:00Z', { channel_id: 'dm-1' })] }],
      pageParams: [undefined],
    });
    readOnScreen(queryClient, 'dm-1');
    await vi.runAllTimersAsync();

    expect(post).toHaveBeenCalledWith('/channels/{ch_id}/read', {
      params: { path: { ch_id: 'dm-1' } },
      body: { message_id: 'd1' },
    });
  });

  it('leaves it unread when the window is not in front or the channel is not open', async () => {
    const queryClient = seeded();
    focused = false;
    readOnScreen(queryClient, 'ch-1');
    focused = true;
    readOnScreen(queryClient, 'ch-other');
    await vi.runAllTimersAsync();

    expect(post).not.toHaveBeenCalled();
  });
});

describe('belongsElsewhere', () => {
  it('knows a channel listed under another workspace, and gives an unknown one the benefit of the doubt', () => {
    useWorkspaceStore.setState({ currentWorkspace: { id: 'ws-1' } as never });
    const queryClient = new QueryClient();
    queryClient.setQueryData(QUERY_KEYS.workspaceChannels('ws-1'), [{ id: 'here' }]);
    queryClient.setQueryData(QUERY_KEYS.workspaceChannels('ws-2'), [{ id: 'there' }]);
    queryClient.setQueryData(QUERY_KEYS.conversations('ws-2'), [{ id: 'dm-there' }]);

    expect(belongsElsewhere(queryClient, 'here')).toBe(false);
    expect(belongsElsewhere(queryClient, 'there')).toBe(true);
    expect(belongsElsewhere(queryClient, 'dm-there')).toBe(true);
    expect(belongsElsewhere(queryClient, 'unknown')).toBe(false);
  });
});
