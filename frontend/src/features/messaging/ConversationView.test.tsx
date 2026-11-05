import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
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
    last_read_at: null,
    participant_ids: ['me'],
    notify_level: null,
    effective_notify_level: 'all',
    departed: [bob],
    ...over,
  };
}

const renderView = (c: Conversation) =>
  render(
    <ConversationView
      workspaceId="ws-1"
      instanceUrl="https://chat.example"
      conversation={c}
      currentUserId="me"
      members={[]}
      channels={[]}
      onClose={vi.fn()}
      onOpenThread={vi.fn()}
      onSave={vi.fn()}
      onForward={vi.fn()}
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

  it('says nothing when everybody is still here', () => {
    renderView(conversation({ participant_ids: ['me', 'ana'], departed: [] }));

    expect(screen.queryByTestId('conversation-departed')).not.toBeInTheDocument();
    expect(screen.getByLabelText('composer')).toBeInTheDocument();
  });
});
