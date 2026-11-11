import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import MessageList from './MessageList';
import { useDisplayStore } from '@/stores/display';
import { isKeptUnread, useKeptUnread } from '@/stores/keptUnread';

const at = (i: number) => new Date(Date.UTC(2026, 0, 2, 12, i)).toISOString();
const message = (id: string, i: number) => ({ id, user_id: 'u1', content: id, created_at: at(i) });

interface Page {
  data: ReturnType<typeof message>[];
  fetch?: number;
  last_read_before?: string;
  last_read_before_at?: string;
}

const served = vi.hoisted(() => ({ pages: null as Page[] | null }));

vi.mock('./hooks/useMessages', () => ({
  useMessages: () => ({
    data: served.pages ? { pages: served.pages } : undefined,
    isLoading: false,
    isError: false,
    isFetching: false,
    isFetchingNextPage: false,
    hasNextPage: false,
    refetch: vi.fn(),
    fetchNextPage: vi.fn(),
  }),
  messagesOldestFirst: (data?: { pages: { data: unknown[] }[] }) => data?.pages.flatMap((p) => p.data) ?? [],
}));
vi.mock('@/features/instance', () => ({ useCurrentUser: () => ({ data: { id: 'u1' } }) }));
vi.mock('./hooks/useMessageLocation', () => ({ useOpenMessage: () => vi.fn() }));
vi.mock('./hooks/useMessageActions', () => ({ useMessageActions: () => ({ currentUserId: 'u1' }) }));
vi.mock('@/stores/users', () => ({ useUserCache: () => ({ getUser: () => undefined }) }));
vi.mock('./MessageItem', () => ({ default: () => null }));
vi.mock('./VirtualMessageList', () => ({
  default: ({ rows, scrollToKey }: { rows: { kind: string; key: string }[]; scrollToKey?: string }) => (
    <div
      data-qa="list"
      data-scroll-to={scrollToKey ?? ''}
      data-divider={rows.find((row) => row.kind === 'unread')?.key ?? ''}
    />
  ),
}));

const history = [message('a', 0), message('b', 1), message('c', 2)];
const page = (fetch: number, lastRead?: string, i = 0): Page => ({
  data: history,
  fetch,
  ...(lastRead ? { last_read_before: lastRead, last_read_before_at: at(i) } : {}),
});

function open() {
  const queryClient = new QueryClient();
  const tree = () => (
    <QueryClientProvider client={queryClient}>
      <MessageList channelId="c-1" onThreadOpen={vi.fn()} markReadIn="w-1" draftKey="c-1" />
    </QueryClientProvider>
  );
  const utils = render(tree());
  return {
    ...utils,
    serve: (pages: Page[]) => {
      served.pages = pages;
      utils.rerender(tree());
    },
  };
}

describe('where a channel opens', () => {
  beforeEach(() => {
    useDisplayStore.setState({ openChannelAt: 'where_left' });
    served.pages = null;
  });

  it('starts at the first unread message, under its line, by default', () => {
    const { serve } = open();
    serve([page(1, 'a')]);
    const list = screen.getByTestId('list');
    expect(list).toHaveAttribute('data-divider', 'unread-b');
    expect(list.dataset.scrollTo).not.toBe('');
  });

  it('starts at the newest message, with no line, when that is the choice', () => {
    useDisplayStore.setState({ openChannelAt: 'newest' });
    const { serve } = open();
    serve([page(1, 'a')]);
    const list = screen.getByTestId('list');
    expect(list).toHaveAttribute('data-divider', '');
    expect(list).toHaveAttribute('data-scroll-to', '');
  });

  it('draws no line from a page fetched before the channel was opened', () => {
    served.pages = [page(4, 'a')];
    open();
    expect(screen.getByTestId('list')).toHaveAttribute('data-divider', '');
  });

  it('takes the line from the fetch that opened the channel, not the one before it', () => {
    served.pages = [page(4, 'a')];
    const { serve } = open();
    serve([page(5, 'b', 1)]);
    expect(screen.getByTestId('list')).toHaveAttribute('data-divider', 'unread-c');
  });

  it('draws no line when the opening fetch found nothing unread', () => {
    served.pages = [page(4, 'a')];
    const { serve } = open();
    serve([page(5)]);
    expect(screen.getByTestId('list')).toHaveAttribute('data-divider', '');
  });

  it('keeps the line where it was while later fetches arrive', () => {
    const { serve } = open();
    serve([page(1, 'a')]);
    serve([page(2)]);
    expect(screen.getByTestId('list')).toHaveAttribute('data-divider', 'unread-b');
  });

  it('lets go of a channel marked unread once it is left', () => {
    useKeptUnread.setState({ channelId: 'c-1' });
    const { unmount } = open();
    expect(isKeptUnread('c-1')).toBe(true);
    unmount();
    expect(isKeptUnread('c-1')).toBe(false);
  });
});
