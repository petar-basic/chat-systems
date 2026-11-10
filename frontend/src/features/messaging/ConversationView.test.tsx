import { describe, it, expect, vi, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, fireEvent } from '@testing-library/react';
import type { WorkspaceMember } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging/hooks/useConversations';
import ConversationView from './ConversationView';

vi.mock('./MessageList', () => ({ default: () => <div data-qa="feed" /> }));
vi.mock('./MessageInput', () => ({ default: () => <div aria-label="composer" role="textbox" /> }));
vi.mock('./OffHoursNote', () => ({ OffHoursNote: () => null }));
vi.mock('@/features/messaging/TypingIndicator', () => ({ default: () => null }));
vi.mock('@/features/messaging/hooks/useMessages', () => ({ useSendMessage: () => ({ mutate: vi.fn() }) }));
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

const renderView = (c: Conversation, members: WorkspaceMember[] = [], onAddPeople = vi.fn()) =>
  render(
    <ConversationView
      workspaceId="ws-1"
      instanceUrl="https://chat.example"
      conversation={c}
      currentUserId="me"
      members={members}
      channels={[]}
      onClose={vi.fn()}
      onOpenThread={vi.fn()}
      onSave={vi.fn()}
      onForward={vi.fn()}
      onAddPeople={onAddPeople}
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
});
