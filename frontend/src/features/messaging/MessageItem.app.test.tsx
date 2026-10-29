import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import MessageItem from './MessageItem';
import { RovingContext, createRovingStore } from './rovingFocus';
import type { Message } from '@/stores/workspace';

vi.mock('@/features/instance/hooks/useAuth', () => ({ useCurrentUser: () => ({ data: { id: 'self' } }) }));
vi.mock('@/features/later/hooks/useLater', () => ({
  useSavedItemId: () => undefined,
  useLaterActions: () => ({ save: { mutate: vi.fn() }, remove: { mutate: vi.fn() } }),
}));
vi.mock('@/features/workspace/hooks/useCurrentWorkspaceRole', () => ({
  useCurrentWorkspaceRole: () => ({ role: 'member' }),
}));
vi.mock('@/features/messaging/UserHoverCard', () => ({
  UserHoverCard: ({ children }: { children: ReactNode }) => children,
}));
vi.mock('./EmojiPicker', () => ({ default: () => null }));
vi.mock('./MessageInput', () => ({ default: () => null }));

const message = (metadata: Message['metadata'] = {}) =>
  ({
    id: 'm-1',
    channel_id: 'ch-1',
    user_id: 'bot-user',
    content: 'deployed',
    created_at: '2026-10-29T10:00:00Z',
    updated_at: '2026-10-29T10:00:00Z',
    reply_count: 0,
    is_pinned: false,
    reactions: [],
    metadata,
  }) as Message;

function renderItem(msg: Message, senderIsApp?: boolean) {
  render(
    <RovingContext value={createRovingStore()}>
      <MessageItem
        message={msg}
        currentUserId="self"
        senderName="Deployer"
        senderIsApp={senderIsApp}
        onToggleReaction={vi.fn()}
        onTogglePin={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />
    </RovingContext>,
  );
}

describe('MessageItem app label', () => {
  it('marks a message from an app bot', () => {
    renderItem(message(), true);
    expect(screen.getByTestId('app-tag')).toHaveTextContent('App');
  });

  it('leaves a person unmarked', () => {
    renderItem(message(), false);
    expect(screen.queryByTestId('app-tag')).not.toBeInTheDocument();
  });

  it('keeps the webhook label for a webhook post', () => {
    renderItem(message({ bot: { hook_id: 'h-1', name: 'CI' } } as Message['metadata']), true);
    expect(screen.getByTestId('bot-tag')).toBeInTheDocument();
    expect(screen.queryByTestId('app-tag')).not.toBeInTheDocument();
  });
});
