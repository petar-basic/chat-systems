import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react';
import { useCommandAnswerStore } from '@/stores/commandAnswer';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging/hooks/useConversations';
import ConversationView from './ConversationView';

const view = vi.hoisted(() => ({
  composer: {} as Record<string, unknown>,
  send: vi.fn(),
  runCommand: vi.fn(),
  convert: vi.fn(),
}));

vi.mock('@/lib/slashCommands', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/slashCommands')>()),
  runCommand: view.runCommand,
}));
vi.mock('./MessageList', () => ({ default: () => <div data-qa="feed" /> }));
vi.mock('./MessageInput', () => ({
  default: (props: Record<string, unknown>) => {
    view.composer = props;
    return <div aria-label="composer" role="textbox" />;
  },
}));
vi.mock('./OffHoursNote', () => ({ OffHoursNote: () => null }));
vi.mock('./hooks/useConversations', () => ({
  useConvertConversation: () => ({ mutateAsync: view.convert, isPending: false }),
}));
vi.mock('@/features/messaging/TypingIndicator', () => ({ default: () => null }));
vi.mock('@/features/messaging/hooks/useMessages', () => ({ useSendMessage: () => ({ mutate: view.send }) }));
vi.mock('./useComposerAttachments', () => ({
  useComposerAttachments: () => ({ add: vi.fn(), take: () => [] }),
  asAttachments: () => [],
}));
vi.mock('@/shared/hooks/useTypingSignal', () => ({
  useTypingSignal: () => ({ signalTyping: vi.fn(), stopTyping: vi.fn() }),
  typingTargetOf: (id: string) => id,
}));
vi.mock('@/shared/components/ConnectionBanner/ConnectionBanner', () => ({ ConnectionBanner: () => null }));
vi.mock('@/features/huddle', () => ({ HuddleStartButton: () => <button type="button">huddle</button> }));
vi.mock('@/features/notifications', () => ({ NotificationLevelButton: () => null }));
vi.mock('@/stores/users', () => ({
  useUserCache: () => ({ getUser: (id: string) => ({ ana: { display_name: 'Ana' } })[id] }),
}));

const bob = { user_id: 'bob', display_name: 'Bob', avatar_url: null, left_at: '2026-11-01T10:00:00Z' };

function conversation(over: Partial<Conversation>): Conversation {
  return {
    id: 'conv-1',
    workspace_id: 'ws-1',
    kind: 'direct',
    last_message_at: '2026-11-02T10:00:00Z',
    unread_count: 0,
    participant_ids: ['me'],
    notify_level: null,
    effective_notify_level: 'all',
    departed: [bob],
    ...over,
  };
}

const member = (user_id: string, display_name: string) =>
  ({
    user_id,
    display_name,
    email: `${user_id}@x.test`,
    avatar_url: null,
    title: null,
    pronouns: null,
  }) as unknown as WorkspaceMember;

const renderView = (
  c: Conversation,
  members: WorkspaceMember[] = [],
  onAddPeople = vi.fn(),
  { canConvert = false, onConverted = vi.fn(), channels = [] as Channel[] } = {},
) =>
  render(
    <ConversationView
      workspaceId="ws-1"
      instanceUrl="https://chat.example"
      conversation={c}
      currentUserId="me"
      members={members}
      channels={channels}
      onClose={vi.fn()}
      onOpenThread={vi.fn()}
      onSave={vi.fn()}
      onForward={vi.fn()}
      onAddPeople={onAddPeople}
      canConvert={canConvert}
      onConverted={onConverted}
    />,
  );

describe('ConversationView', () => {
  it('keeps a DM with somebody who left readable, under their name, with nothing to write in', () => {
    renderView(conversation({}));

    expect(screen.getByTestId('conversation-title')).toHaveTextContent('Bob');
    expect(screen.getByText('Bob is no longer in this workspace')).toBeInTheDocument();
    expect(screen.getByTestId('feed')).toBeInTheDocument();
    expect(screen.queryByLabelText('composer')).not.toBeInTheDocument();
    expect(screen.queryByText('huddle')).not.toBeInTheDocument();
  });

  it('lets a group carry on, saying who left above the composer', () => {
    renderView(conversation({ kind: 'group', participant_ids: ['me', 'ana'] }));

    expect(screen.getByTestId('conversation-title')).toHaveTextContent('Ana, Bob');
    expect(screen.getByText('Bob is no longer in this workspace')).toBeInTheDocument();
    expect(screen.getByLabelText('composer')).toBeInTheDocument();
  });

  it('adds people by starting a new group with everybody already here', () => {
    const onAddPeople = vi.fn();
    const people = [member('me', 'Me'), member('ana', 'Ana'), member('cy', 'Cy')];
    renderView(conversation({ participant_ids: ['me', 'ana'], departed: [] }), people, onAddPeople);

    fireEvent.click(screen.getByRole('button', { name: 'Add people' }));
    expect(screen.getByRole('heading', { name: 'Add people to this conversation' })).toBeInTheDocument();
    const candidates = screen.getAllByTestId('new-dm-candidate');
    expect(candidates.map((c) => c.dataset.userId)).toEqual(['cy']);
    fireEvent.click(candidates[0]);
    fireEvent.click(screen.getByTestId('new-dm-start'));

    expect(onAddPeople).toHaveBeenCalledWith(['ana', 'cy']);
  });

  it('offers nobody to add when the other person left or the group is full', () => {
    const { unmount } = renderView(conversation({}));
    expect(screen.queryByRole('button', { name: 'Add people' })).not.toBeInTheDocument();
    unmount();

    const full = ['me', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
    renderView(conversation({ kind: 'group', participant_ids: full, departed: [] }));
    expect(screen.queryByRole('button', { name: 'Add people' })).not.toBeInTheDocument();
  });

  it('says nothing when everybody is still here', () => {
    renderView(conversation({ participant_ids: ['me', 'ana'], departed: [] }));

    expect(screen.queryByTestId('conversation-departed')).not.toBeInTheDocument();
    expect(screen.getByLabelText('composer')).toBeInTheDocument();
  });
});

describe('a group DM that becomes a channel', () => {
  const group = conversation({ kind: 'group', participant_ids: ['me', 'ana', 'cy'], departed: [] });

  beforeEach(() => {
    view.convert.mockReset();
  });

  it('is offered for a group of three to someone who may make private channels', () => {
    const { unmount } = renderView(group);
    expect(screen.queryByTestId('conversation-convert')).not.toBeInTheDocument();
    unmount();

    const pair = renderView(
      conversation({ kind: 'group', participant_ids: ['me', 'ana'], departed: [] }),
      [],
      vi.fn(),
      {
        canConvert: true,
      },
    );
    expect(screen.queryByTestId('conversation-convert')).not.toBeInTheDocument();
    pair.unmount();

    const direct = renderView(conversation({ participant_ids: ['me', 'ana'], departed: [] }), [], vi.fn(), {
      canConvert: true,
    });
    expect(screen.queryByTestId('conversation-convert')).not.toBeInTheDocument();
    direct.unmount();

    renderView(group, [], vi.fn(), { canConvert: true });
    expect(screen.getByRole('button', { name: 'Convert to a private channel' })).toBeInTheDocument();
  });

  it('takes a channel name and opens the channel it became', async () => {
    const onConverted = vi.fn();
    const channel = { id: 'conv-1', name: 'launch-plan' } as Channel;
    view.convert.mockResolvedValue(channel);
    renderView(group, [], vi.fn(), { canConvert: true, onConverted });

    fireEvent.click(screen.getByTestId('conversation-convert'));
    fireEvent.change(screen.getByTestId('conversation-convert-name'), { target: { value: 'Launch Plan' } });
    fireEvent.click(screen.getByTestId('conversation-convert-submit'));

    await waitFor(() => expect(onConverted).toHaveBeenCalledWith(channel));
    expect(view.convert).toHaveBeenCalledWith({ conversationId: 'conv-1', name: 'launch-plan' });
  });

  it('refuses a name a channel already has before asking the server', () => {
    const channels = [{ id: 'ch-1', name: 'launch' } as Channel];
    renderView(group, [], vi.fn(), { canConvert: true, channels });

    fireEvent.click(screen.getByTestId('conversation-convert'));
    fireEvent.change(screen.getByTestId('conversation-convert-name'), { target: { value: 'Launch' } });
    fireEvent.click(screen.getByTestId('conversation-convert-submit'));

    expect(screen.getByText('A channel with that name already exists.')).toBeInTheDocument();
    expect(view.convert).not.toHaveBeenCalled();
  });
});

describe('a command typed in a conversation', () => {
  const typed = (content: string) =>
    act(() => (view.composer.onSend as (c: string) => Promise<void>)(content));

  beforeEach(() => {
    view.send.mockReset();
    view.runCommand.mockReset();
    useCommandAnswerStore.setState({ answers: {} });
  });

  it('runs instead of being sent, and answers only here', async () => {
    view.runCommand.mockResolvedValue({ response_type: 'ephemeral', text: 'You are away' });
    renderView(conversation({ participant_ids: ['me', 'ana'], departed: [] }));
    expect(view.composer.slashCommands).toBe(true);

    await typed('/away');

    expect(view.runCommand).toHaveBeenCalledWith('conv-1', '/away', 'https://chat.example', undefined);
    expect(view.send).not.toHaveBeenCalled();
    expect(screen.getByTestId('command-response')).toHaveTextContent('You are away');
  });

  it('keeps an unknown command in the composer and says so', async () => {
    view.runCommand.mockResolvedValue(null);
    renderView(conversation({ participant_ids: ['me', 'ana'], departed: [] }));

    await expect(typed('/nosuch thing')).rejects.toThrow();

    expect(view.send).not.toHaveBeenCalled();
    expect(await screen.findByTestId('command-response')).toHaveTextContent('/nosuch is not a command here');
  });

  it('sends a message that is not a command', async () => {
    renderView(conversation({ participant_ids: ['me', 'ana'], departed: [] }));

    await typed('hello');

    expect(view.runCommand).not.toHaveBeenCalled();
    expect(view.send).toHaveBeenCalledWith(expect.objectContaining({ content: 'hello' }));
  });
});

describe('ConversationView in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('counts a group and says who left, with names as text', async () => {
    const hostile = '<img src=x onerror="alert(1)"> {{name}}';
    await i18n.changeLanguage('ja');
    renderView(
      conversation({
        kind: 'group',
        participant_ids: ['me', 'ana'],
        departed: [{ ...bob, display_name: hostile }],
      }),
    );
    expect(screen.getByRole('main', { name: 'ダイレクトメッセージ' })).toBeInTheDocument();
    expect(screen.getByTestId('conversation-title').textContent).toBe(`Ana、${hostile}`);
    expect(screen.getByText('2人')).toBeInTheDocument();
    expect(screen.getByText(`${hostile} さんはこのワークスペースにいません`)).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('adds people by starting a new group', async () => {
    const people = [member('me', 'Me'), member('ana', 'Ana'), member('cy', 'Cy')];
    await i18n.changeLanguage('ja');
    renderView(conversation({ participant_ids: ['me', 'ana'], departed: [] }), people, vi.fn());
    fireEvent.click(screen.getByRole('button', { name: 'メンバーを追加' }));
    expect(screen.getByRole('heading', { name: 'この会話にメンバーを追加' })).toBeInTheDocument();
    expect(
      screen.getByText(
        '新しいグループチャットを始めます。今の会話はそのまま残ります。あと 7 人まで追加できます。',
      ),
    ).toBeInTheDocument();
    fireEvent.click(screen.getAllByTestId('new-dm-candidate')[0]);
    expect(screen.getByTestId('new-dm-start').textContent).toBe('追加');
  });

  it('converts a group into a private channel', async () => {
    const group = conversation({ kind: 'group', participant_ids: ['me', 'ana', 'cy'], departed: [] });
    const channels = [{ id: 'ch-1', name: 'launch' } as Channel];
    view.convert.mockReset().mockRejectedValue({});
    await i18n.changeLanguage('ja');
    renderView(group, [], vi.fn(), { canConvert: true, channels });

    fireEvent.click(screen.getByRole('button', { name: 'プライベートチャンネルに変換' }));
    expect(screen.getByRole('heading', { name: 'プライベートチャンネルに変換' })).toBeInTheDocument();
    expect(
      screen.getByText(
        'このグループチャットの全員が、これまでの会話をすべて引き継いだプライベートチャンネルのメンバーになり、あなたがその管理者になります。チャンネルをグループチャットに戻すことはできません。',
      ),
    ).toBeInTheDocument();
    expect(screen.getByTestId('conversation-convert-submit').textContent).toBe('変換する');

    fireEvent.change(screen.getByTestId('conversation-convert-name'), { target: { value: 'Launch' } });
    fireEvent.click(screen.getByTestId('conversation-convert-submit'));
    expect(screen.getByText('同じ名前のチャンネルがすでにあります。')).toBeInTheDocument();

    fireEvent.change(screen.getByTestId('conversation-convert-name'), { target: { value: 'Launch Plan' } });
    fireEvent.click(screen.getByTestId('conversation-convert-submit'));
    expect(await screen.findByText('この会話を変換できませんでした')).toBeInTheDocument();
  });
});
