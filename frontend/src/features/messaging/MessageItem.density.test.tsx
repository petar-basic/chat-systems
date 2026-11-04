import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import MessageItem from './MessageItem';
import { RovingContext, createRovingStore } from './rovingFocus';
import { useDisplayStore } from '@/stores/display';
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

const message = {
  id: 'm-1',
  channel_id: 'ch-1',
  user_id: 'ana',
  content: 'shipping now',
  created_at: '2026-10-29T10:00:00Z',
  updated_at: '2026-10-29T10:00:00Z',
  reply_count: 0,
  is_pinned: false,
  reactions: [],
  metadata: {},
} as unknown as Message;

function renderItem(grouped: boolean) {
  render(
    <RovingContext value={createRovingStore()}>
      <MessageItem
        message={message}
        currentUserId="self"
        senderName="Ana"
        grouped={grouped}
        onToggleReaction={vi.fn()}
        onTogglePin={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
        dataQa="row"
      />
    </RovingContext>,
  );
  return screen.getByTestId('row');
}

describe('MessageItem density', () => {
  beforeEach(() => {
    useDisplayStore.setState({ density: 'clean', previews: 'shown' });
  });

  it('puts the name above the message and leaves it off a grouped one when clean', () => {
    const row = renderItem(false);
    expect(within(row).getByTestId('message-sender')).toHaveTextContent('Ana');
    expect(within(row).queryByTestId('message-compact-sender')).not.toBeInTheDocument();
    expect(within(row).queryByTestId('message-time')).not.toBeInTheDocument();
  });

  it('puts the time and the name on the line of every message when compact', () => {
    useDisplayStore.setState({ density: 'compact' });
    const row = renderItem(true);

    expect(within(row).getByTestId('message-time')).toBeInTheDocument();
    const lead = within(row).getByTestId('message-compact-sender');
    expect(within(lead).getByTestId('message-sender')).toHaveTextContent('Ana');
    expect(row).toHaveTextContent('shipping now');
    expect(row.className).toContain('py-0.5');
  });
});
