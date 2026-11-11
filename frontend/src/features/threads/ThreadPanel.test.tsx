import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ReactNode } from 'react';
import '@/lib/i18n';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { globalEventBus } from '@/lib/globalEventBus';
import { useCommandAnswerStore } from '@/stores/commandAnswer';
import type { Channel, Message } from '@/stores/workspace';
import ThreadPanel from './ThreadPanel';

const panel = vi.hoisted(() => ({
  replies: [] as unknown[],
  composer: {} as Record<string, unknown>,
  fetched: undefined as unknown,
  sendReply: vi.fn(),
  runCommand: vi.fn(),
}));

vi.mock('@/lib/slashCommands', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/slashCommands')>()),
  runCommand: panel.runCommand,
}));

vi.mock('@/features/threads/hooks/useThreads', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/threads/hooks/useThreads')>()),
  useThreadMessages: () => ({
    data: panel.replies,
    isLoading: false,
    hasNextPage: false,
    fetchNextPage: vi.fn(),
    isFetchingNextPage: false,
  }),
  useThreadState: () => ({ data: undefined, refetch: vi.fn() }),
  useFollowThread: () => ({ mutate: vi.fn(), isPending: false }),
  useSendThreadReply: () => ({ mutate: panel.sendReply }),
  useThreadReplyActions: () => ({
    toggleReaction: vi.fn(),
    edit: vi.fn(),
    remove: vi.fn(),
    togglePin: vi.fn(),
  }),
}));
vi.mock('@/features/messaging', async () => ({
  useSlashCommand: (await import('@/features/messaging/hooks/useSlashCommand')).useSlashCommand,
  CommandAnswer: (await import('@/features/messaging/CommandAnswer')).CommandAnswer,
  RichTextDisplay: ({ content }: { content: string }) => <p>{content}</p>,
  MessageInput: (props: Record<string, unknown>) => {
    panel.composer = props;
    return null;
  },
  MessageItem: () => null,
  isGrouped: () => false,
  asAttachments: () => [],
  useComposerAttachments: () => ({ take: () => [] }),
  GalleryProvider: ({ children }: { children: ReactNode }) => children,
}));
vi.mock('@/features/channel', () => ({
  useChannelModeration: () => ({ canBroadcast: false, members: [] }),
  useChannelPreview: (channelId?: string) => ({ data: channelId ? panel.fetched : undefined }),
}));

const at = (i: number) => new Date(Date.UTC(2026, 10, 11, 9, i)).toISOString();
const root = {
  id: 'root',
  channel_id: 'ch-1',
  user_id: 'u2',
  content: 'the plan',
  metadata: {},
  thread_parent_id: null,
  reply_count: 0,
  is_pinned: false,
  created_at: at(0),
  updated_at: at(0),
  deleted_at: null,
} as unknown as Message;
const design = { id: 'ch-1', name: 'design', channel_type: 'public', is_archived: false } as Channel;

function openPanel(channels: Channel[] = [design]) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ThreadPanel parentMessage={root} channels={channels} onClose={vi.fn()} />
    </QueryClientProvider>,
  );
}

describe('the thread panel', () => {
  beforeEach(() => {
    panel.replies = [];
    panel.composer = {};
    panel.fetched = undefined;
    panel.sendReply.mockReset();
    panel.runCommand.mockReset();
    useCommandAnswerStore.setState({ answers: {} });
  });

  it('closes a thread whose root is deleted while it is open and has no replies', () => {
    openPanel();
    expect(screen.getByTestId('thread-also-to-channel')).toBeInTheDocument();

    act(() => {
      globalEventBus.emit({ type: 'message.deleted', message_id: 'root', channel_id: 'ch-1' });
    });

    expect(screen.getByTestId('thread-parent-deleted')).toBeInTheDocument();
    expect(screen.getByTestId('thread-closed')).toHaveTextContent(
      'This thread was deleted, so it takes no new replies.',
    );
    expect(screen.queryByTestId('thread-also-to-channel')).toBeNull();
  });

  it('ignores the deletion of another message', () => {
    openPanel();

    act(() => {
      globalEventBus.emit({ type: 'message.deleted', message_id: 'other', channel_id: 'ch-1' });
    });

    expect(screen.queryByTestId('thread-closed')).toBeNull();
    expect(screen.getByText('the plan')).toBeInTheDocument();
  });

  it('clears the box to also send to the channel once a reply is scheduled', () => {
    openPanel();
    fireEvent.click(screen.getByLabelText('Also send to #design'));
    expect(panel.composer.scheduleTarget).toEqual(expect.objectContaining({ alsoToChannel: true }));

    act(() => (panel.composer.onScheduled as () => void)());

    expect(screen.getByLabelText('Also send to #design')).not.toBeChecked();
    expect(panel.composer.scheduleTarget).toEqual(expect.objectContaining({ alsoToChannel: false }));
  });

  it('runs a command typed in the thread in that thread, and answers there', async () => {
    panel.runCommand.mockResolvedValue({ response_type: 'ephemeral', text: 'You are away' });
    openPanel();
    expect(panel.composer.slashCommands).toBe(true);

    await act(() => (panel.composer.onSend as (content: string) => Promise<void>)('/away'));

    expect(panel.runCommand).toHaveBeenCalledWith('ch-1', '/away', undefined, 'root');
    expect(panel.sendReply).not.toHaveBeenCalled();
    expect(screen.getByTestId('command-response')).toHaveTextContent('You are away');
    expect(useCommandAnswerStore.getState().answers['ch-1']).toBeUndefined();
  });

  it('shows nothing once a command posts its answer in the thread', async () => {
    panel.runCommand.mockResolvedValue({ response_type: 'in_channel', text: '' });
    openPanel();

    await act(() => (panel.composer.onSend as (content: string) => Promise<void>)('/shrug fine'));

    expect(panel.sendReply).not.toHaveBeenCalled();
    expect(screen.queryByTestId('command-response')).toBeNull();
  });

  it('sends a reply that is not a command', async () => {
    openPanel();

    await act(() => (panel.composer.onSend as (content: string) => Promise<void>)('on it'));

    expect(panel.runCommand).not.toHaveBeenCalled();
    expect(panel.sendReply).toHaveBeenCalledWith(expect.objectContaining({ content: 'on it' }));
  });

  it('takes no reply in a thread of an archived channel the sidebar does not carry', () => {
    panel.fetched = { ...design, is_archived: true };
    openPanel([]);

    expect(screen.getByTestId('thread-archived')).toHaveTextContent(
      '#design is archived, so its threads take no new replies.',
    );
    expect(screen.queryByTestId('thread-also-to-channel')).toBeNull();
  });
});
