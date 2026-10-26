import { useState } from 'react';
import { MessageSquare, Pencil, Trash2 } from 'lucide-react';
import { MessageInput, RichTextDisplay } from '@/features/messaging';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { formatDateTime } from '@/lib/datetime';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import type { ListComment } from './listModel';
import type { People } from './people';
import { useCommentActions, useItemComments } from './useLists';

interface Props {
  itemId: string;
  instanceUrl?: string;
  currentUserId: string;
  canComment: boolean;
  canModerate: boolean;
  people: People;
  members: WorkspaceMember[];
  channels: Channel[];
}

export function ItemComments({
  itemId,
  instanceUrl,
  currentUserId,
  canComment,
  canModerate,
  people,
  members,
  channels,
}: Props) {
  const comments = useItemComments(itemId, instanceUrl);
  const actions = useCommentActions(itemId, instanceUrl);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<ListComment | null>(null);
  const mentionable = members.filter((m) => people.eligible.some((p) => p.id === m.user_id));
  const thread = comments.data ?? [];

  return (
    <section aria-label="Comments" data-qa="item-comments" className="border-t border-line/50 pt-4">
      <h3 className="mb-2 flex items-center gap-1.5 text-xs font-medium text-muted">
        <MessageSquare className="h-3.5 w-3.5" />
        Comments
        {thread.length > 0 && <span className="tabular-nums text-subtle">{thread.length}</span>}
      </h3>
      {comments.isLoading ? (
        <div className="flex justify-center py-3 text-accent">
          <Spinner />
        </div>
      ) : thread.length === 0 ? (
        <p className="py-1 text-xs text-subtle" data-qa="item-comments-empty">
          No comments yet.
        </p>
      ) : (
        <ul className="space-y-3">
          {thread.map((comment) => {
            const mine = comment.user_id === currentUserId;
            return (
              <li key={comment.id} data-qa="item-comment" className="group">
                <div className="flex items-center gap-2 text-xs">
                  <span className="font-semibold text-fg-soft" data-qa="item-comment-author">
                    {people.nameOf(comment.user_id)}
                  </span>
                  <time dateTime={comment.created_at} className="text-subtle">
                    {formatDateTime(comment.created_at)}
                  </time>
                  {comment.edited_at && <span className="text-subtle">(edited)</span>}
                  <span className="ml-auto flex gap-0.5 opacity-0 transition group-hover:opacity-100 focus-within:opacity-100">
                    {mine && canComment && editingId !== comment.id && (
                      <IconButton
                        label="Edit comment"
                        icon={<Pencil className="h-3.5 w-3.5" />}
                        onClick={() => setEditingId(comment.id)}
                        data-qa="item-comment-edit"
                      />
                    )}
                    {((mine && canComment) || canModerate) && (
                      <IconButton
                        label="Delete comment"
                        icon={<Trash2 className="h-3.5 w-3.5" />}
                        tone="danger"
                        onClick={() => setDeleting(comment)}
                        data-qa="item-comment-delete"
                      />
                    )}
                  </span>
                </div>
                {editingId === comment.id ? (
                  <MessageInput
                    key={`comment-edit:${comment.id}`}
                    editing
                    initialContent={comment.content}
                    members={mentionable}
                    channels={channels}
                    onSend={async (content) => {
                      await actions.edit.mutateAsync({ commentId: comment.id, content });
                      setEditingId(null);
                    }}
                    onCancel={() => setEditingId(null)}
                  />
                ) : (
                  <div className="text-sm text-fg-soft" data-qa="item-comment-content">
                    <RichTextDisplay content={comment.content} compact />
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {canComment && (
        <div className="mt-3" data-qa="item-comment-composer">
          <MessageInput
            placeholder="Add a comment…"
            members={mentionable}
            channels={channels}
            draftKey={`list-comment:${itemId}`}
            onSend={async (content) => {
              await actions.add.mutateAsync(content);
            }}
          />
        </div>
      )}
      {deleting && (
        <ConfirmDialog
          title="Delete this comment?"
          description="It will be removed for everyone."
          confirmLabel="Delete comment"
          onConfirm={() => actions.remove.mutate(deleting.id)}
          onClose={() => setDeleting(null)}
          dataQa="item-comment-delete-confirm"
        />
      )}
    </section>
  );
}
