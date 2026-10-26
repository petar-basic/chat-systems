import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, render, screen, fireEvent } from '@testing-library/react';
import type { ReactNode } from 'react';
import MessageItem from './MessageItem';
import { RovingContext, createRovingStore } from './rovingFocus';
import { editLastOwnMessage } from './messageKeys';
import type { Message } from '@/stores/workspace';

const SELF = 'user-self';
const removeLater = vi.fn();
let savedItemId: string | undefined;

vi.mock('@/features/instance/hooks/useAuth', () => ({ useCurrentUser: () => ({ data: { id: SELF } }) }));
vi.mock('@/features/later/hooks/useLater', () => ({
  useSavedItemId: () => savedItemId,
  useLaterActions: () => ({ save: { mutate: vi.fn() }, remove: { mutate: removeLater } }),
}));
vi.mock('@/features/workspace/hooks/useCurrentWorkspaceRole', () => ({
  useCurrentWorkspaceRole: () => ({ role: 'member' }),
}));
vi.mock('@/features/messaging/UserHoverCard', () => ({
  UserHoverCard: ({ children }: { children: ReactNode }) => children,
}));
vi.mock('./EmojiPicker', () => ({ default: () => <div data-qa="emoji-picker" /> }));
vi.mock('./MessageInput', () => ({ default: () => <div data-qa="edit-box" /> }));

function message(id: string, userId: string, extra: Partial<Message> = {}): Message {
  return {
    id,
    channel_id: 'ch-1',
    user_id: userId,
    content: `message ${id}`,
    created_at: '2026-10-22T10:00:00Z',
    updated_at: '2026-10-22T10:00:00Z',
    reply_count: 0,
    is_pinned: false,
    reactions: [],
    metadata: {},
    ...extra,
  } as Message;
}

const thread = vi.fn();
const pin = vi.fn();
const save = vi.fn();

function renderPane(messages: Message[]) {
  const store = createRovingStore();
  store.setFallback(messages.at(-1)?.id ?? null);
  render(
    <RovingContext value={store}>
      <div data-message-pane>
        {messages.map((m) => (
          <MessageItem
            key={m.id}
            message={m}
            currentUserId={SELF}
            senderName="Ana"
            onThreadOpen={thread}
            onToggleReaction={vi.fn()}
            onTogglePin={pin}
            onEdit={vi.fn()}
            onDelete={vi.fn()}
            onSave={save}
          />
        ))}
        <div data-composer>
          <div contentEditable="true" tabIndex={0} data-qa="composer" />
        </div>
      </div>
    </RovingContext>,
  );
  return screen.getAllByTestId('message-row');
}

describe('MessageItem keyboard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    savedItemId = undefined;
  });

  it('is one Tab stop that follows the focused message', () => {
    const [first, second] = renderPane([message('m1', 'other'), message('m2', SELF)]);
    expect(first).toHaveAttribute('tabindex', '-1');
    expect(second).toHaveAttribute('tabindex', '0');
    second.focus();
    fireEvent.keyDown(second, { key: 'ArrowUp' });
    expect(first).toHaveFocus();
    expect(first).toHaveAttribute('tabindex', '0');
    expect(second).toHaveAttribute('tabindex', '-1');
    fireEvent.keyDown(first, { key: 'ArrowDown' });
    expect(second).toHaveFocus();
  });

  it('acts on the focused message', () => {
    const [row] = renderPane([message('m1', 'other', { is_pinned: true })]);
    fireEvent.keyDown(row, { key: 't' });
    fireEvent.keyDown(row, { key: 'Enter' });
    expect(thread).toHaveBeenCalledTimes(2);
    fireEvent.keyDown(row, { key: 'p' });
    expect(pin).toHaveBeenCalledWith('m1', true);
    fireEvent.keyDown(row, { key: 'a' });
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ id: 'm1' }));
    fireEvent.keyDown(row, { key: 'r' });
    expect(screen.getByTestId('emoji-picker')).toBeInTheDocument();
  });

  it('takes a saved message off Later with A', () => {
    savedItemId = 'later-1';
    const [row] = renderPane([message('m1', 'other')]);
    fireEvent.keyDown(row, { key: 'a' });
    expect(removeLater).toHaveBeenCalledWith('later-1');
    expect(save).not.toHaveBeenCalled();
  });

  it('edits only your own message with E', () => {
    const [theirs, mine] = renderPane([message('m1', 'other'), message('m2', SELF)]);
    expect(fireEvent.keyDown(theirs, { key: 'e' })).toBe(true);
    expect(screen.queryByTestId('edit-box')).not.toBeInTheDocument();
    fireEvent.keyDown(mine, { key: 'e' });
    expect(screen.getByTestId('edit-box')).toBeInTheDocument();
  });

  it('goes back to the message box with Esc', () => {
    const [row] = renderPane([message('m1', 'other')]);
    row.focus();
    fireEvent.keyDown(row, { key: 'Escape' });
    expect(screen.getByTestId('composer')).toHaveFocus();
  });

  it('ignores keys meant for a control inside the message', () => {
    renderPane([message('m1', 'other')]);
    fireEvent.keyDown(screen.getByLabelText('Reply in thread'), { key: 't' });
    expect(thread).not.toHaveBeenCalled();
  });

  it('opens your last message for editing from the message box', () => {
    renderPane([message('m1', SELF), message('m2', 'other'), message('m3', SELF, { pending: true })]);
    let opened = false;
    act(() => {
      opened = editLastOwnMessage(screen.getByTestId('composer'));
    });
    expect(opened).toBe(true);
    expect(screen.getAllByTestId('edit-box')).toHaveLength(1);
    expect(screen.getAllByTestId('message-row')[0]).toContainElement(screen.getByTestId('edit-box'));
  });
});
