import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import i18n from '@/lib/i18n';
import MessageItem from './MessageItem';
import { RovingContext, createRovingStore } from './rovingFocus';
import { useDisplayStore } from '@/stores/display';
import { useUserCache } from '@/stores/users';
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

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const base = {
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

function renderItem(message: Message) {
  render(
    <RovingContext value={createRovingStore()}>
      <MessageItem
        message={message}
        currentUserId="self"
        senderName="Ana"
        grouped={false}
        onThreadOpen={vi.fn()}
        onToggleReaction={vi.fn()}
        onTogglePin={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
        onRetry={vi.fn()}
        onDiscard={vi.fn()}
        dataQa="row"
      />
    </RovingContext>,
  );
  return screen.queryByTestId('row') ?? document.body;
}

describe('MessageItem in Japanese', () => {
  beforeEach(async () => {
    useDisplayStore.setState({ density: 'clean', previews: 'shown' });
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says a message was deleted and counts its replies', () => {
    renderItem({ ...base, deleted_at: '2026-10-29T11:00:00Z', reply_count: 3 } as Message);
    expect(screen.getByTestId('message-deleted')).toHaveTextContent('このメッセージは削除されました');
    expect(screen.getByTestId('message-thread-open')).toHaveTextContent('返信 3件');
  });

  it('marks an edited and pinned message, and counts one reply', () => {
    const row = renderItem({ ...base, updated_at: '2026-10-29T10:05:00Z', is_pinned: true, reply_count: 1 });
    expect(within(row).getByTestId('pinned-marker')).toHaveTextContent('ピン留め');
    expect(within(row).getByTestId('edited-marker')).toHaveTextContent('(編集済み)');
    expect(within(row).getByTestId('message-thread-open')).toHaveTextContent('返信 1件スレッドを表示');
  });

  it('says a message failed to send and offers to retry or discard it', () => {
    const row = renderItem({ ...base, failed: true } as Message);
    expect(within(row).getByTestId('message-failed')).toHaveTextContent('送信できませんでした。再送信破棄');
  });

  it('names who reacted, with names as text', () => {
    useUserCache.setState({
      users: new Map([['bob', { id: 'bob', email: 'bob@x.test', display_name: hostile, avatar_url: null }]]),
    });
    renderItem({
      ...base,
      reactions: [
        { emoji: '👍', user_id: 'bob' },
        { emoji: '👍', user_id: 'self' },
      ],
    } as Message);
    expect(screen.getByTestId('message-reaction')).toHaveAccessibleName(
      `あなた と ${hostile} がリアクションしました`,
    );
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
