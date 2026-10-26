import { memo, useEffect, useState, useRef } from 'react';
import { MessageSquare, Pin, Trash2 } from 'lucide-react';
import { UserHoverCard } from '@/features/messaging/UserHoverCard';
import { useLongPress } from '@/shared/hooks/useLongPress';
import MessageActionSheet from './MessageActionSheet';
import type { Message, WorkspaceMember, Channel } from '@/stores/workspace';
import RichTextDisplay from '@/features/messaging/RichTextDisplay';
import type { PollKeyVote } from '@/features/polls';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { HuddleSystemMessage } from '@/features/huddle';
import {
  EDIT_MESSAGE_EVENT,
  focusComposer,
  messageKeyAction,
  moveFocus,
  stepMessage,
  type MessageKeyAction,
} from './messageKeys';
import { useIsTabStop, useRovingFocus } from './rovingFocus';
import { useLaterActions, useSavedItemId } from '@/features/later';
import { useWorkspaceStore } from '@/stores/workspace';
import EditHistoryPanel from './EditHistoryPanel';
import { useCurrentWorkspaceRole } from '@/features/workspace';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { QUICK_REACTIONS, sheetActionsFor, type MessageActionHandlers } from './messageActionList';
import { MessageActionsBar } from './MessageActionsBar';
import { MessageBody } from './MessageBody';
import { MessageEditor } from './MessageEditor';
import { MessageReactions } from './MessageReactions';

interface MessageItemProps {
  message: Message;
  currentUserId: string;
  senderName: string;
  senderStatusEmoji?: string | null;
  senderStatusText?: string | null;
  senderAvatarUrl?: string | null;
  isHighlighted?: boolean;
  grouped?: boolean;
  members?: WorkspaceMember[];
  channels?: Channel[];
  onThreadOpen?: (msg: Message) => void;
  onToggleReaction: (messageId: string, emoji: string, hasOwn: boolean) => void;
  onTogglePin: (messageId: string, isPinned: boolean) => void;
  onEdit: (messageId: string, content: string, attachmentIds?: string[]) => Promise<unknown> | void;
  onDelete: (messageId: string) => Promise<unknown> | void;
  onHidePreviews?: (messageId: string) => void;
  onRefreshPreviews?: (messageId: string) => void;
  onMarkUnread?: (messageId: string) => void;
  onRetry?: (messageId: string, content: string) => void;
  onCopyLink?: (messageId: string) => void;
  onSave?: (message: Message) => void;
  onForward?: (message: Message) => void;
  dataQa?: string;
}

function MessageItem({
  message,
  currentUserId,
  senderName,
  senderStatusEmoji,
  senderStatusText,
  senderAvatarUrl,
  isHighlighted,
  grouped,
  members,
  channels,
  onThreadOpen,
  onToggleReaction,
  onTogglePin,
  onEdit,
  onDelete,
  onHidePreviews,
  onRefreshPreviews,
  onMarkUnread,
  onRetry,
  onCopyLink,
  onSave,
  onForward,
  dataQa = 'message-row',
}: MessageItemProps) {
  const [editing, setEditing] = useState(false);
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const savedItemId = useSavedItemId(onSave ? workspace?.id : undefined, message.id, workspace?.instanceUrl);
  const later = useLaterActions(workspace?.id, workspace?.instanceUrl);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);

  const isOwn = currentUserId === message.user_id;
  const isEdited = message.updated_at !== message.created_at;
  const canEdit = isOwn && !message.metadata?.poll && !message.metadata?.form;
  const pollKeyVote = useRef<PollKeyVote | null>(null);
  const [showHistory, setShowHistory] = useState(false);
  const rowRef = useRef<HTMLDivElement>(null);
  const isTabStop = useIsTabStop(message.id);
  const focusRow = useRovingFocus();
  const [sheetOpen, setSheetOpen] = useState(false);
  const longPress = useLongPress(() => setSheetOpen(true), !editing && !confirmDelete);

  const toggleSave = onSave
    ? () => {
        if (savedItemId) later.remove.mutate(savedItemId);
        else onSave(message);
      }
    : undefined;
  const actions: MessageActionHandlers = {
    saved: !!savedItemId,
    isOwn,
    canEdit,
    onThread: onThreadOpen ? () => onThreadOpen(message) : undefined,
    onToggleSave: toggleSave,
    onRemind: onSave ? (at) => later.save.mutate({ messageId: message.id, remindAt: at }) : undefined,
    onCopyLink: onCopyLink ? () => onCopyLink(message.id) : undefined,
    onMarkUnread: onMarkUnread ? () => onMarkUnread(message.id) : undefined,
    onForward: onForward ? () => onForward(message) : undefined,
    onTogglePin: () => onTogglePin(message.id, message.is_pinned),
    onEdit: () => setEditing(true),
    onDelete: () => setConfirmDelete(true),
  };

  useEffect(() => {
    const row = rowRef.current;
    if (!row || !canEdit || message.pending) return undefined;
    const onEdit = (event: Event) => {
      event.preventDefault();
      setEditing(true);
    };
    row.addEventListener(EDIT_MESSAGE_EVENT, onEdit);
    return () => row.removeEventListener(EDIT_MESSAGE_EVENT, onEdit);
  }, [canEdit, message.pending]);

  useEffect(() => {
    if (!editing) return;
    rowRef.current?.scrollIntoView({ block: 'nearest' });
  }, [editing]);
  // The marker becomes a control only for people entitled to what is behind it;
  // for everyone else it stays the plain marker it has always been.
  const { role } = useCurrentWorkspaceRole();
  const canSeeHistory = message.user_id === currentUserId || role === 'admin' || role === 'owner';

  if (message.metadata?.kind === 'huddle_started' && message.metadata.huddle_id && !message.deleted_at) {
    return (
      <HuddleSystemMessage
        channelId={message.channel_id}
        huddleId={message.metadata.huddle_id}
        initiatorId={message.metadata.initiator_id ?? message.user_id}
      />
    );
  }
  const time = new Date(message.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const attachments = message.metadata?.attachments ?? [];

  const finishEditing = () => {
    const row = rowRef.current;
    if (row?.contains(document.activeElement)) focusComposer(row);
    setEditing(false);
  };

  const handleEditSave = async (content: string, kept?: string[]) => {
    await onEdit(message.id, content, kept);
    finishEditing();
  };

  const keyHandlers: Partial<Record<MessageKeyAction, () => void>> = {
    edit: canEdit && !message.pending ? () => setEditing(true) : undefined,
    thread: actions.onThread,
    react: () => setShowEmojiPicker(true),
    later: toggleSave,
    pin: message.pending ? undefined : actions.onTogglePin,
  };

  const handleRowKey = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget || editing || e.nativeEvent.isComposing) return;
    const row = e.currentTarget;
    const pollIndex = /^[1-9]$/.test(e.key) && !e.metaKey && !e.ctrlKey && !e.altKey ? Number(e.key) - 1 : -1;
    if (pollIndex >= 0 && pollKeyVote.current) {
      pollKeyVote.current(pollIndex);
      e.preventDefault();
      return;
    }
    const action = messageKeyAction(e);
    if (!action) return;
    let handled: boolean;
    if (action === 'previous' || action === 'next') {
      handled = moveFocus(stepMessage(row, action === 'next' ? 1 : -1));
    } else if (action === 'composer') {
      handled = focusComposer(row);
    } else {
      const handler = keyHandlers[action];
      handler?.();
      handled = !!handler;
    }
    if (handled) e.preventDefault();
  };

  const handleReactionToggle = (emoji: string) => {
    const hasOwn = (message.reactions ?? []).some((r) => r.emoji === emoji && r.user_id === currentUserId);
    onToggleReaction(message.id, emoji, hasOwn);
  };

  if (message.deleted_at) {
    return (
      <div
        data-message-id={message.id}
        data-qa="message-deleted"
        className="flex items-start gap-3 py-1.5 px-2 rounded-lg opacity-50"
      >
        <div className="w-8 h-8 rounded-full bg-raised flex items-center justify-center text-sm shrink-0 mt-0.5">
          <Trash2 className="w-3.5 h-3.5 text-muted" />
        </div>
        <div className="flex-1 min-w-0 py-1">
          <p className="text-sm text-muted italic">This message was deleted</p>
        </div>
      </div>
    );
  }

  const pinnedMarker = message.is_pinned ? (
    <span data-qa="pinned-marker" className="text-xs text-warning inline-flex items-center gap-0.5">
      <Pin className="w-3 h-3" /> pinned
    </span>
  ) : null;

  const editedMarker =
    isEdited && !message.pending ? (
      canSeeHistory ? (
        <button
          onClick={() => setShowHistory((open) => !open)}
          data-qa="edited-marker"
          aria-expanded={showHistory}
          className="text-xs text-muted italic underline decoration-dotted hover:text-fg-soft transition cursor-pointer"
        >
          (edited)
        </button>
      ) : (
        <span data-qa="edited-marker" className="text-xs text-muted italic">
          (edited)
        </span>
      )
    ) : null;

  return (
    <>
      <div
        ref={rowRef}
        data-message-id={message.id}
        data-message-row
        data-own={isOwn && !message.pending}
        data-qa={dataQa}
        tabIndex={isTabStop ? 0 : -1}
        onFocus={(e) => {
          if (e.target === e.currentTarget) focusRow(message.id);
        }}
        onKeyDown={handleRowKey}
        {...longPress}
        onClick={(e) => {
          if (!e.altKey || !onMarkUnread || message.pending || editing) return;
          e.preventDefault();
          onMarkUnread(message.id);
        }}
        className={`group relative flex items-start gap-3 px-2 rounded-lg transition-colors hover:bg-surface/50 focus-visible:outline-none focus-visible:bg-surface/50 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-purple-500 ${grouped ? 'py-0.5' : 'py-1.5'} ${message.pending ? 'opacity-50' : ''} ${isHighlighted ? 'bg-amber-500/10 ring-1 ring-inset ring-amber-500/25' : ''}`}
      >
        {grouped ? (
          <div className="w-8 shrink-0 flex justify-end pr-0.5">
            <span className="text-[10px] leading-5 text-muted opacity-0 group-hover:opacity-100 tabular-nums">
              {time}
            </span>
          </div>
        ) : (
          <UserHoverCard
            userId={message.user_id}
            name={senderName}
            avatarUrl={senderAvatarUrl}
            triggerClassName="inline-flex shrink-0 cursor-pointer"
          >
            <Avatar
              userId={message.user_id}
              name={senderName}
              avatarUrl={senderAvatarUrl}
              className="mt-0.5"
            />
          </UserHoverCard>
        )}
        <div className="flex-1 min-w-0">
          {!grouped && (
            <div className="flex items-baseline gap-2">
              <UserHoverCard userId={message.user_id} name={senderName} avatarUrl={senderAvatarUrl}>
                <button
                  type="button"
                  data-qa="message-sender"
                  className="text-sm font-semibold text-fg-soft hover:underline cursor-pointer"
                >
                  {senderName}
                </button>
              </UserHoverCard>
              {senderStatusEmoji && (
                <span data-qa="message-status-emoji" title={senderStatusText ?? undefined}>
                  {senderStatusEmoji}
                </span>
              )}
              {message.metadata?.bot && (
                <span
                  data-qa="bot-tag"
                  className="px-1 py-0.5 text-[10px] font-semibold uppercase tracking-wide bg-raised text-fg-dim rounded"
                >
                  {message.metadata.bot.workflow_id ? 'Workflow' : 'Bot'}
                </span>
              )}
              <span className="text-xs text-muted">{time}</span>
              {message.pending && <span className="text-xs text-muted italic">Sending…</span>}
              {editedMarker}
              {pinnedMarker}
            </div>
          )}

          {editing ? (
            <MessageEditor
              message={message}
              members={members}
              channels={channels}
              onSave={handleEditSave}
              onCancel={finishEditing}
            />
          ) : (
            <MessageBody
              message={message}
              currentUserId={currentUserId}
              pollKeyVote={pollKeyVote}
              markers={
                grouped && (editedMarker || pinnedMarker) ? (
                  <>
                    {editedMarker}
                    {pinnedMarker}
                  </>
                ) : undefined
              }
              onHidePreviews={onHidePreviews}
              onRefreshPreviews={onRefreshPreviews}
            />
          )}

          {showHistory && (
            <EditHistoryPanel
              messageId={message.id}
              currentContent={message.content}
              onClose={() => setShowHistory(false)}
            />
          )}

          {message.failed && (
            <div className="mt-1 flex items-center gap-2 text-xs text-danger" data-qa="message-failed">
              <span>Failed to send.</span>
              {onRetry && (
                <button
                  onClick={() => onRetry(message.id, message.content)}
                  data-qa="message-retry"
                  className="font-semibold text-danger hover:text-danger underline"
                >
                  Retry
                </button>
              )}
            </div>
          )}

          {message.reply_count > 0 && onThreadOpen && (
            <button
              onClick={() => onThreadOpen(message)}
              data-qa="message-thread-open"
              className="mt-1 flex items-center gap-1.5 text-xs text-accent hover:text-accent-soft transition group/thread"
            >
              <MessageSquare className="w-3.5 h-3.5" />
              <span className="font-medium">
                {message.reply_count} {message.reply_count === 1 ? 'reply' : 'replies'}
              </span>
              <span className="text-muted group-hover/thread:text-accent transition">View thread</span>
            </button>
          )}

          <MessageReactions message={message} currentUserId={currentUserId} onToggle={handleReactionToggle} />
        </div>

        {!editing && !confirmDelete && (
          <MessageActionsBar
            message={message}
            actions={actions}
            pickerOpen={showEmojiPicker}
            onPickerChange={setShowEmojiPicker}
            onReact={handleReactionToggle}
          />
        )}

        {sheetOpen && (
          <MessageActionSheet
            quickReactions={QUICK_REACTIONS}
            onReact={(emoji) => handleReactionToggle(emoji)}
            actions={sheetActionsFor(message, actions)}
            onClose={() => setSheetOpen(false)}
          />
        )}
      </div>
      {confirmDelete && (
        <ConfirmDialog
          title="Delete message?"
          description="This can't be undone."
          confirmLabel="Delete"
          onConfirm={async () => {
            await onDelete(message.id);
          }}
          onClose={() => setConfirmDelete(false)}
          dataQa="message-delete"
        >
          <div
            className="rounded-lg border border-line bg-raised/30 px-3 py-2.5"
            data-qa="message-delete-preview"
          >
            <div className="mb-1 flex items-baseline gap-2">
              <span className="text-sm font-semibold text-fg">{senderName}</span>
              <span className="text-xs text-muted">{time}</span>
            </div>
            <RichTextDisplay content={message.content} attachments={attachments} />
          </div>
        </ConfirmDialog>
      )}
    </>
  );
}

export default memo(MessageItem);
