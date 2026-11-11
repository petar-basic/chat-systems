import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { ReactNode } from 'react';
import '@/lib/i18n';
import { render, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { Message } from '@/stores/workspace';
import ThreadPanel from './ThreadPanel';

const thread = vi.hoisted(() => ({ replies: [] as unknown[], markRead: vi.fn() }));

vi.mock('@/features/threads/hooks/useThreads', () => ({
  useLiveMessage: (message: unknown) => message,
  useThreadMessages: () => ({
    data: thread.replies,
    isLoading: false,
    hasNextPage: false,
    fetchNextPage: vi.fn(),
    isFetchingNextPage: false,
  }),
  useThreadState: () => ({ data: undefined, refetch: thread.markRead }),
  useFollowThread: () => ({ mutate: vi.fn(), isPending: false }),
  useSendThreadReply: () => ({ mutate: vi.fn() }),
  useThreadReplyActions: () => ({
    toggleReaction: vi.fn(),
    edit: vi.fn(),
    remove: vi.fn(),
    togglePin: vi.fn(),
  }),
}));
vi.mock('@/features/messaging', () => ({
  RichTextDisplay: () => null,
  MessageInput: () => null,
  MessageItem: () => null,
  isGrouped: () => false,
  asAttachments: () => [],
  useComposerAttachments: () => ({ take: () => [] }),
  GalleryProvider: ({ children }: { children: ReactNode }) => children,
}));
vi.mock('@/features/channel', () => ({
  useChannelModeration: () => ({ canBroadcast: false, members: [] }),
}));

const at = (i: number) => new Date(Date.UTC(2026, 10, 11, 9, i)).toISOString();
const message = (id: string, i: number, parent: string | null = 'root') =>
  ({
    id,
    channel_id: 'ch-1',
    user_id: 'u2',
    content: id,
    metadata: {},
    thread_parent_id: parent,
    reply_count: 0,
    is_pinned: false,
    created_at: at(i),
    updated_at: at(i),
    deleted_at: null,
  }) as unknown as Message;

const root = message('root', 0, null);

function openPanel() {
  const queryClient = new QueryClient();
  const tree = () => (
    <QueryClientProvider client={queryClient}>
      <ThreadPanel parentMessage={root} onClose={vi.fn()} />
    </QueryClientProvider>
  );
  const utils = render(tree());
  return {
    arrive: (reply: Message) => {
      thread.replies = [...thread.replies, reply];
      utils.rerender(tree());
    },
  };
}

describe('the thread panel', () => {
  let focused = true;
  beforeEach(() => {
    focused = true;
    vi.spyOn(document, 'hasFocus').mockImplementation(() => focused);
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    thread.replies = [message('r1', 1)];
    thread.markRead.mockReset();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('reads a reply that arrives while you are looking at it', () => {
    const { arrive } = openPanel();
    arrive(message('r2', 2));
    expect(thread.markRead).toHaveBeenCalledTimes(1);
  });

  it('leaves a reply unread while the window is in the background, and reads it on return', () => {
    const { arrive } = openPanel();
    focused = false;
    act(() => {
      window.dispatchEvent(new Event('blur'));
    });
    arrive(message('r2', 2));
    arrive(message('r3', 3));
    expect(thread.markRead).not.toHaveBeenCalled();

    focused = true;
    act(() => {
      window.dispatchEvent(new Event('focus'));
    });

    expect(thread.markRead).toHaveBeenCalledTimes(1);
  });
});
