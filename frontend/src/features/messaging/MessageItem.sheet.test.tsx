import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, render, screen, fireEvent } from '@testing-library/react';
import type { ReactNode } from 'react';
import MessageItem from './MessageItem';
import { RovingContext, createRovingStore } from './rovingFocus';
import type { Message } from '@/stores/workspace';

const { success } = vi.hoisted(() => ({ success: vi.fn() }));

vi.mock('@/features/instance/hooks/useAuth', () => ({ useCurrentUser: () => ({ data: { id: 'self' } }) }));
vi.mock('@/features/later/hooks/useLater', () => ({
  useSavedItemId: () => undefined,
  useLaterActions: () => ({ save: { mutate: vi.fn() }, remove: { mutate: vi.fn() } }),
}));
vi.mock('@/features/workspace/hooks/useCurrentWorkspaceRole', () => ({
  useCurrentWorkspaceRole: () => ({ role: 'member' }),
}));
vi.mock('@/features/apps', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/apps')>()),
  useMessageShortcuts: () => ({ data: [] }),
  useRunAppShortcut: () => ({ mutateAsync: vi.fn() }),
}));
vi.mock('@/shared/components/Toast', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/shared/components/Toast')>()),
  toast: { success, error: vi.fn() },
}));
vi.mock('@/features/messaging/UserHoverCard', () => ({
  UserHoverCard: ({ children }: { children: ReactNode }) => children,
}));
vi.mock('./EmojiPicker', () => ({ default: () => null }));
vi.mock('./MessageInput', () => ({ default: () => null }));

const message = (content: string, extra: Partial<Message> = {}) =>
  ({
    id: 'm-1',
    channel_id: 'ch-1',
    user_id: 'other',
    content,
    created_at: '2026-11-24T10:00:00Z',
    updated_at: '2026-11-24T10:00:00Z',
    reply_count: 0,
    is_pinned: false,
    reactions: [],
    metadata: {},
    ...extra,
  }) as Message;

function holdMessage(msg: Message) {
  render(
    <RovingContext value={createRovingStore()}>
      <MessageItem
        message={msg}
        currentUserId="self"
        senderName="Ana"
        onToggleReaction={vi.fn()}
        onTogglePin={vi.fn()}
        onEdit={vi.fn()}
        onDelete={vi.fn()}
      />
    </RovingContext>,
  );
  const row = screen.getByTestId('message-row');
  fireEvent.pointerDown(row, { pointerType: 'touch', clientX: 20, clientY: 20 });
  act(() => {
    vi.advanceTimersByTime(500);
  });
  fireEvent.pointerUp(row, { pointerType: 'touch', clientX: 20, clientY: 20 });
  return screen.getByTestId('message-action-sheet');
}

describe('MessageItem action sheet', () => {
  const writeText = vi.fn();

  beforeEach(() => {
    vi.useFakeTimers();
    writeText.mockReset();
    writeText.mockResolvedValue(undefined);
    success.mockReset();
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('copies the text of a message as it reads', async () => {
    const sheet = holdMessage(message('**ship** it, @[Ana Horvat](u-1)\n\n- today'));

    fireEvent.click(sheet.querySelector('[data-qa="sheet-action-copy-text"]')!);
    await act(async () => {});

    expect(writeText).toHaveBeenCalledWith('ship it, @Ana Horvat\n• today');
    expect(success).toHaveBeenCalledWith('Text copied to clipboard');
    expect(screen.queryByTestId('message-action-sheet')).not.toBeInTheDocument();
  });

  it('offers no text to copy from a message that has none', () => {
    const sheet = holdMessage(message('  '));

    expect(sheet.querySelector('[data-qa="sheet-action-copy-text"]')).toBeNull();
  });
});
