import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import MessageList from './MessageList';
import { useDisplayStore } from '@/stores/display';

const at = (i: number) => new Date(Date.UTC(2026, 0, 2, 12, i)).toISOString();
const message = (id: string, i: number) => ({ id, user_id: 'u1', content: id, created_at: at(i) });

vi.mock('./hooks/useMessages', () => ({
  useMessages: () => ({
    data: { pages: [{ data: [message('a', 0), message('b', 1), message('c', 2)] }] },
    isLoading: false,
    isError: false,
    isFetching: false,
    isFetchingNextPage: false,
    hasNextPage: false,
    refetch: vi.fn(),
    fetchNextPage: vi.fn(),
  }),
  messagesOldestFirst: (data: { pages: { data: unknown[] }[] }) => data.pages.flatMap((p) => p.data),
}));
vi.mock('@/features/channel', () => ({
  useUnreadChannels: () => ({
    data: { counts: [{ channel_id: 'c-1', unread_count: 2, last_read_msg: 'a', last_read_at: at(0) }] },
  }),
}));
vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { id: 'w-1', instanceUrl: 'http://chat.test' } }),
}));
vi.mock('./hooks/useMessageLocation', () => ({ useOpenMessage: () => vi.fn() }));
vi.mock('./hooks/useMessageActions', () => ({ useMessageActions: () => ({ currentUserId: 'u1' }) }));
vi.mock('@/stores/users', () => ({ useUserCache: () => ({ getUser: () => undefined }) }));
vi.mock('./MessageItem', () => ({ default: () => null }));
vi.mock('./VirtualMessageList', () => ({
  default: ({ rows, scrollToKey }: { rows: { kind: string; key: string }[]; scrollToKey?: string }) => (
    <div
      data-qa="list"
      data-scroll-to={scrollToKey ?? ''}
      data-divider={rows.some((row) => row.kind === 'unread') ? 'yes' : 'no'}
    />
  ),
}));

function open() {
  const queryClient = new QueryClient();
  render(
    <QueryClientProvider client={queryClient}>
      <MessageList channelId="c-1" onThreadOpen={vi.fn()} markReadIn="w-1" draftKey="c-1" />
    </QueryClientProvider>,
  );
  return screen.getByTestId('list');
}

describe('where a channel opens', () => {
  beforeEach(() => {
    useDisplayStore.setState({ openChannelAt: 'where_left' });
  });

  it('starts at the first unread message, under its line, by default', () => {
    const list = open();
    expect(list).toHaveAttribute('data-divider', 'yes');
    expect(list.dataset.scrollTo).not.toBe('');
  });

  it('starts at the newest message, with no line, when that is the choice', () => {
    useDisplayStore.setState({ openChannelAt: 'newest' });
    const list = open();
    expect(list).toHaveAttribute('data-divider', 'no');
    expect(list).toHaveAttribute('data-scroll-to', '');
  });
});
