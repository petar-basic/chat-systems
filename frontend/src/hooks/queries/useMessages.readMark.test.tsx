import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ReactNode } from 'react';
import { renderHook, waitFor, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MESSAGES_PAGE_SIZE, QUERY_KEYS } from '@/shared/constants';
import { useMessages } from './useMessages';
import type { AppNotification } from './useNotifications';

const get = vi.fn();
const client = {
  typed: async (call: (c: { GET: typeof get }) => Promise<{ data: unknown }>) =>
    (await call({ GET: get })).data,
};

vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => client,
  useCurrentApi: () => client,
}));

const page = Array.from({ length: MESSAGES_PAGE_SIZE }, (_, i) => ({ id: `msg-${i}`, channel_id: 'ch-1' }));

function notification(id: string, channelId: string): AppNotification {
  return {
    id,
    workspace_id: 'ws-1',
    user_id: 'u-1',
    notification_type: 'mention',
    title: 'You were mentioned',
    body: '',
    channel_id: channelId,
    message_id: null,
    list_id: null,
    item_id: null,
    is_read: false,
    created_at: '2026-10-26T09:00:00Z',
  };
}

function setup(markReadIn?: string) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  queryClient.setQueryData(QUERY_KEYS.notifications('ws-1'), [
    notification('n-1', 'ch-1'),
    notification('n-2', 'ch-2'),
  ]);
  queryClient.setQueryData(QUERY_KEYS.notificationUnreadCount('ws-1'), 5);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const view = renderHook(() => useMessages('ch-1', markReadIn), { wrapper });
  return { queryClient, view };
}

function queryOf(call: unknown[]) {
  return (call[1] as { params: { query: Record<string, unknown> } }).params.query;
}

describe('useMessages read mark', () => {
  beforeEach(() => {
    get.mockReset();
    get.mockImplementation(
      async (_path: string, { params }: { params: { query: { mark_read?: boolean } } }) => ({
        data: params.query.mark_read ? { data: page, unread_notifications: 1 } : { data: [] },
      }),
    );
  });

  it('asks the first page to mark the channel read and settles the notifications from its answer', async () => {
    const { queryClient, view } = setup('ws-1');
    await waitFor(() => expect(view.result.current.isSuccess).toBe(true));

    expect(queryOf(get.mock.calls[0])).toMatchObject({ mark_read: true, cursor: undefined });
    expect(queryClient.getQueryData(QUERY_KEYS.notificationUnreadCount('ws-1'))).toBe(1);
    expect(
      queryClient.getQueryData<AppNotification[]>(QUERY_KEYS.notifications('ws-1'))?.map((n) => n.is_read),
    ).toEqual([true, false]);

    await act(() => view.result.current.fetchNextPage());
    expect(get).toHaveBeenCalledTimes(2);
    expect(queryOf(get.mock.calls[1])).toMatchObject({
      mark_read: undefined,
      cursor: `msg-${MESSAGES_PAGE_SIZE - 1}`,
    });
  });

  it('leaves the read position alone where nobody asked for it', async () => {
    const { queryClient, view } = setup();
    await waitFor(() => expect(view.result.current.isSuccess).toBe(true));

    expect(queryOf(get.mock.calls[0]).mark_read).toBeUndefined();
    expect(queryClient.getQueryData(QUERY_KEYS.notificationUnreadCount('ws-1'))).toBe(5);
  });
});
