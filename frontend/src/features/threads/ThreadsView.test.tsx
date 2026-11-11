import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ThreadSummary } from '@/features/threads/hooks/useThreads';
import { useCommandAnswerStore } from '@/stores/commandAnswer';
import { ThreadsView } from './ThreadsView';

const markRead = vi.fn();
let composer: Record<string, unknown> = {};
const sendReply = vi.fn();
const runCommand = vi.hoisted(() => vi.fn());
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

const place = (overrides: Partial<ThreadSummary['channel']> = {}): ThreadSummary['channel'] => ({
  name: 'design',
  channel_type: 'public',
  is_archived: false,
  is_member: true,
  ...overrides,
});

const thread = (unread: number, channel = place()): ThreadSummary => ({
  root: { ...message('root', 'u1', 'the plan', '2026-10-20T09:00:00Z', null), reply_count: 5 },
  channel,
  latest_replies: [
    message('r3', 'u1', 'my reply', '2026-10-21T09:00:00Z'),
    message('r4', 'u2', 'second answer', '2026-10-22T09:00:00Z'),
    message('r5', 'u2', 'third answer', '2026-10-22T10:00:00Z'),
  ],
  unread_count: unread,
  last_read_at: '2026-10-21T09:00:00Z',
  last_reply_at: '2026-10-22T10:00:00Z',
});

vi.mock('@/features/threads/hooks/useThreads', () => ({
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

vi.mock('@/lib/slashCommands', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/slashCommands')>()),
  runCommand,
}));

vi.mock('@/features/messaging/MessageInput', () => ({
  default: (props: { onSend: (content: string) => Promise<void> }) => {
    composer = props;
    return (
      <button type="button" onClick={() => void props.onSend('on it')}>
        send
      </button>
    );
  },
}));

vi.mock('@/features/messaging/useComposerAttachments', () => ({
  useComposerAttachments: () => ({ take: () => [] }),
  asAttachments: () => [],
}));

const onOpenThread = vi.fn();
const onOpenChannel = vi.fn();
const onOpenConversation = vi.fn();

const sidebar = (name = 'design') => [{ id: 'ch-1', name, channel_type: 'public' } as never];

const renderView = (channels: never[] = sidebar()) =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ThreadsView
        workspaceId="ws-1"
        currentUserId="u1"
        channels={channels}
        conversations={[]}
        members={[]}
        onOpenThread={onOpenThread}
        onOpenChannel={onOpenChannel}
        onOpenConversation={onOpenConversation}
      />
    </QueryClientProvider>,
  );

describe('ThreadsView', () => {
  let focused = true;
  beforeEach(() => {
    focused = true;
    vi.spyOn(document, 'hasFocus').mockImplementation(() => focused);
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    markRead.mockClear();
    sendReply.mockClear();
    runCommand.mockReset();
    useCommandAnswerStore.setState({ answers: {} });
    onOpenThread.mockClear();
    onOpenChannel.mockClear();
    onOpenConversation.mockClear();
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

  it('leaves a thread unread while the window is in the background, and reads it on return', () => {
    focused = false;
    threads = [thread(2)];
    renderView();
    expect(screen.getByTestId('thread-new')).toHaveTextContent('2 new replies');
    expect(markRead).not.toHaveBeenCalled();

    focused = true;
    act(() => {
      window.dispatchEvent(new Event('focus'));
    });

    expect(markRead).toHaveBeenCalledTimes(1);
    expect(markRead).toHaveBeenCalledWith('root');
  });

  it('replies in place', async () => {
    threads = [thread(0)];
    renderView();
    await act(async () => fireEvent.click(screen.getByText('send')));
    expect(runCommand).not.toHaveBeenCalled();
    expect(sendReply).toHaveBeenCalledWith(expect.objectContaining({ content: 'on it' }), expect.anything());
  });

  it('runs a command typed in a card in that thread instead of replying with it', async () => {
    threads = [thread(0)];
    runCommand.mockResolvedValue({ response_type: 'ephemeral', text: 'You are away' });
    renderView();
    expect(composer.slashCommands).toBe(true);

    await act(() => (composer.onSend as (content: string) => Promise<void>)('/away'));

    expect(runCommand).toHaveBeenCalledWith('ch-1', '/away', undefined, 'root');
    expect(sendReply).not.toHaveBeenCalled();
    expect(screen.getByTestId('command-response')).toHaveTextContent('You are away');
  });

  it('schedules and sends to the channel as the thread panel does', async () => {
    threads = [thread(0)];
    renderView();
    expect(composer.scheduleTarget).toEqual({
      channelId: 'ch-1',
      threadParentId: 'root',
      alsoToChannel: false,
    });

    fireEvent.click(screen.getByLabelText('Also send to #design'));
    expect(composer.scheduleTarget).toEqual(expect.objectContaining({ alsoToChannel: true }));
    await act(async () => fireEvent.click(screen.getByText('send')));
    expect(sendReply).toHaveBeenCalledWith(
      expect.objectContaining({ content: 'on it', alsoToChannel: true }),
      expect.anything(),
    );
  });

  it('clears the box to also send to the channel once a reply is scheduled', () => {
    threads = [thread(0)];
    renderView();
    fireEvent.click(screen.getByLabelText('Also send to #design'));

    act(() => (composer.onScheduled as () => void)());

    expect(screen.getByLabelText('Also send to #design')).not.toBeChecked();
    expect(composer.scheduleTarget).toEqual(expect.objectContaining({ alsoToChannel: false }));
  });

  it('names a thread in an archived channel, takes no reply there, and opens the channel', () => {
    threads = [thread(0, place({ is_archived: true }))];
    renderView([]);
    const card = screen.getByTestId('thread-card');
    expect(card).toHaveAccessibleName('Thread in design');
    expect(screen.queryByTestId('thread-card-reply')).toBeNull();
    expect(screen.getByTestId('thread-card-archived')).toHaveTextContent(
      '#design is archived, so its threads take no new replies.',
    );

    fireEvent.click(screen.getByRole('button', { name: 'design' }));

    expect(onOpenChannel).toHaveBeenCalledWith('ch-1');
    expect(onOpenConversation).not.toHaveBeenCalled();
  });

  it('names a thread in a public channel you left and opens that channel', () => {
    threads = [thread(0, place({ is_member: false }))];
    renderView([]);
    expect(screen.getByTestId('thread-card')).toHaveAccessibleName('Thread in design');
    expect(screen.getByTestId('thread-card-reply')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'design' }));

    expect(onOpenChannel).toHaveBeenCalledWith('ch-1');
  });

  it('opens a thread in a direct message as that conversation', () => {
    threads = [thread(0, place({ name: null, channel_type: 'dm' }))];
    renderView([]);
    expect(screen.getByLabelText('Also send to the conversation')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Direct message' }));

    expect(onOpenConversation).toHaveBeenCalledWith('ch-1');
    expect(onOpenChannel).not.toHaveBeenCalled();
  });

  it('says so when nothing is followed', () => {
    threads = [];
    renderView();
    expect(screen.getByTestId('threads-empty')).toBeInTheDocument();
  });
});

describe('ThreadsView in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names each thread and the channel to also send to, as text', async () => {
    const hostile = '<img src=x onerror="alert(1)"> {{name}}';
    threads = [thread(2, place({ name: hostile }))];
    await i18n.changeLanguage('ja');
    renderView(sidebar(hostile));
    expect(screen.getByRole('heading', { name: 'スレッド' })).toBeInTheDocument();
    expect(screen.getByTestId('thread-card')).toHaveAccessibleName(`${hostile} のスレッド`);
    expect(screen.getByLabelText(`#${hostile} にも投稿`)).toBeInTheDocument();
    expect(screen.getByTestId('thread-new')).toHaveTextContent('新しい返信 2件');
    expect(screen.getByTestId('thread-open')).toHaveTextContent('残りの返信 2件を表示');
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
