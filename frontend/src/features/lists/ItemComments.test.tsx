import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import type { WorkspaceMember } from '@/stores/workspace';
import { ItemComments } from './ItemComments';
import { peopleFrom } from './people';
import type { ListComment } from './listModel';

const add = vi.fn<(content: string) => Promise<ListComment>>();
const edit = vi.fn<(vars: { commentId: string; content: string }) => Promise<ListComment>>();
const remove = vi.fn<(commentId: string) => void>();
const thread: { data: ListComment[] } = { data: [] };
const composers: Array<{ members: WorkspaceMember[] }> = [];

vi.mock('./useLists', () => ({
  useItemComments: () => ({ data: thread.data, isLoading: false }),
  useCommentActions: () => ({
    add: { mutateAsync: add },
    edit: { mutateAsync: edit },
    remove: { mutate: remove },
  }),
}));

vi.mock('@/features/messaging/MessageInput', () => ({
  default: (props: {
    members: WorkspaceMember[];
    initialContent?: string;
    editing?: boolean;
    onSend: (content: string) => Promise<void>;
  }) => {
    composers.push({ members: props.members });
    return (
      <button
        data-qa={props.editing ? 'fake-edit-send' : 'fake-send'}
        onClick={() =>
          void props.onSend(props.editing ? `${props.initialContent} (fixed)` : 'hello @[Bob](u-bob)')
        }
      >
        send
      </button>
    );
  },
}));

const member = (user_id: string, display_name: string) =>
  ({ user_id, display_name, email: `${display_name}@x.dev`, role: 'member' }) as WorkspaceMember;
const MEMBERS = [member('u-ana', 'Ana'), member('u-bob', 'Bob'), member('u-cy', 'Cy')];

const comment = (
  id: string,
  user_id: string,
  content: string,
  edited_at: string | null = null,
): ListComment => ({
  id,
  item_id: 'i-1',
  list_id: 'l-1',
  user_id,
  content,
  created_at: '2026-10-20T10:00:00Z',
  edited_at,
});

function renderComments({ canComment = true, canModerate = false } = {}) {
  render(
    <ItemComments
      itemId="i-1"
      currentUserId="u-ana"
      canComment={canComment}
      canModerate={canModerate}
      people={peopleFrom(MEMBERS, ['u-ana', 'u-bob'])}
      members={MEMBERS}
      channels={[]}
    />,
  );
}

const byAuthor = (name: string) =>
  screen
    .getAllByTestId('item-comment')
    .find((c) => within(c).getByTestId('item-comment-author').textContent === name)!;

beforeEach(() => {
  add.mockReset();
  edit.mockReset();
  remove.mockReset();
  composers.length = 0;
  thread.data = [comment('k-1', 'u-ana', 'mine'), comment('k-2', 'u-bob', 'theirs', '2026-10-20T11:00:00Z')];
});

describe('ItemComments', () => {
  it('shows the thread by author and marks what was edited', () => {
    renderComments();
    expect(screen.getAllByTestId('item-comment-content').map((c) => c.textContent)).toEqual([
      'mine',
      'theirs',
    ]);
    expect(byAuthor('Bob')).toHaveTextContent('(edited)');
    expect(byAuthor('Ana')).not.toHaveTextContent('(edited)');
  });

  it('posts a comment and offers only the people who can see the list to mention', () => {
    renderComments();
    fireEvent.click(screen.getByTestId('fake-send'));
    expect(add).toHaveBeenCalledWith('hello @[Bob](u-bob)');
    expect(composers.at(-1)?.members.map((m) => m.user_id)).toEqual(['u-ana', 'u-bob']);
  });

  it('lets the author edit and delete their own comment, and nobody else’s', () => {
    renderComments();
    expect(within(byAuthor('Bob')).queryByTestId('item-comment-edit')).toBeNull();
    expect(within(byAuthor('Bob')).queryByTestId('item-comment-delete')).toBeNull();
    fireEvent.click(within(byAuthor('Ana')).getByTestId('item-comment-edit'));
    fireEvent.click(screen.getByTestId('fake-edit-send'));
    expect(edit).toHaveBeenCalledWith({ commentId: 'k-1', content: 'mine (fixed)' });
  });

  it('lets a moderator remove anyone’s comment after confirming', () => {
    renderComments({ canModerate: true });
    fireEvent.click(within(byAuthor('Bob')).getByTestId('item-comment-delete'));
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('item-comment-delete-confirm-confirm'));
    expect(remove).toHaveBeenCalledWith('k-2');
  });

  it('reads without a composer when the list is read-only', () => {
    renderComments({ canComment: false });
    expect(screen.queryByTestId('item-comment-composer')).toBeNull();
    expect(screen.queryByTestId('item-comment-edit')).toBeNull();
    expect(screen.queryByTestId('item-comment-delete')).toBeNull();
  });

  it('says when nobody has commented yet', () => {
    thread.data = [];
    renderComments();
    expect(screen.getByTestId('item-comments-empty')).toHaveTextContent('No comments yet.');
  });
});
