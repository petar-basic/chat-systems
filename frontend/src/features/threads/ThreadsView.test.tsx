import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ThreadSummary } from '@/hooks/queries/useThreads';
import { ThreadsView } from './ThreadsView';

const markRead = vi.fn();
const sendReply = vi.fn();
let threads: ThreadSummary[] = [];

const message = (id: string, user: string, content: string, at: string, parent: string | null = 'root') => ({
  id,
  channel_id: 'ch-1',
  user_id: user,
  client_message_id: null,
  content,
  metadata: {},
  thread_parent_id: parent,
  reply_count: 0,
  is_pinned: false,
  created_at: at,
  updated_at: at,
  deleted_at: null,
});

const thread = (unread: number): ThreadSummary => ({
  root: { ...message('root', 'u1', 'the plan', '2026-10-20T09:00:00Z', null), reply_count: 5 },
  latest_replies: [
    message('r3', 'u1', 'my reply', '2026-10-21T09:00:00Z'),
    message('r4', 'u2', 'second answer', '2026-10-22T09:00:00Z'),
    message('r5', 'u2', 'third answer', '2026-10-22T10:00:00Z'),
  ],
  unread_count: unread,
  last_read_at: '2026-10-21T09:00:00Z',
  last_reply_at: '2026-10-22T10:00:00Z',
});

vi.mock('@/hooks/queries/useThreads', () => ({
  useFollowedThreads: () => ({
    data: { pages: [{ data: threads, next_cursor: null }] },
    isLoading: false,
    isError: false,
    isSuccess: true,
    hasNextPage: false,
  }),
  useMarkThreadRead: () => ({ mutate: markRead }),
  useSendThreadReply: () => ({ mutate: sendReply }),
}));

vi.mock('@/features/messaging', () => ({
  MessageInput: ({ onSend }: { onSend: (content: string) => Promise<void> }) => (
    <button type="button" onClick={() => void onSend('on it')}>
      send
    </button>
  ),
}));

vi.mock('@/features/messaging/useComposerAttachments', () => ({
  useComposerAttachments: () => ({ take: () => [] }),
  asAttachments: () => [],
}));

const onOpenThread = vi.fn();

const renderView = () =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ThreadsView
        workspaceId="ws-1"
        currentUserId="u1"
        channels={[{ id: 'ch-1', name: 'design', channel_type: 'public' } as never]}
        conversations={[]}
        members={[]}
        onOpenThread={onOpenThread}
        onOpenChannel={vi.fn()}
        onOpenConversation={vi.fn()}
      />
    </QueryClientProvider>,
  );

describe('ThreadsView', () => {
  beforeEach(() => {
    markRead.mockClear();
    sendReply.mockClear();
    onOpenThread.mockClear();
  });

  it('shows each thread with its root, latest replies and the rest a click away', () => {
    threads = [thread(0)];
    renderView();
    const card = screen.getByTestId('thread-card');
    expect(card).toHaveTextContent('design');
    expect(card).toHaveTextContent('the plan');
    expect(card).toHaveTextContent('third answer');
    fireEvent.click(screen.getByTestId('thread-open'));
    expect(onOpenThread).toHaveBeenCalledWith(expect.objectContaining({ id: 'root' }));
    expect(screen.queryByTestId('thread-new')).toBeNull();
    expect(markRead).not.toHaveBeenCalled();
  });

  it('marks new replies, counts them and reads the thread once', () => {
    threads = [thread(2)];
    renderView();
    expect(screen.getByTestId('thread-new')).toHaveTextContent('2 new replies');
    expect(screen.getAllByTestId('thread-reply-new')).toHaveLength(2);
    expect(markRead).toHaveBeenCalledTimes(1);
    expect(markRead).toHaveBeenCalledWith('root');
  });

  it('replies in place', () => {
    threads = [thread(0)];
    renderView();
    fireEvent.click(screen.getByText('send'));
    expect(sendReply).toHaveBeenCalledWith(expect.objectContaining({ content: 'on it' }), expect.anything());
  });

  it('says so when nothing is followed', () => {
    threads = [];
    renderView();
    expect(screen.getByTestId('threads-empty')).toBeInTheDocument();
  });
});
