import { memo, useEffect, useState, useRef } from 'react';
import {
  Pencil,
  Trash2,
  MessageSquare,
  SmilePlus,
  Pin,
  Link2,
  Bookmark,
  Forward,
  MoreHorizontal,
} from 'lucide-react';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { useLongPress } from '@/shared/hooks/useLongPress';
import MessageActionSheet, { type SheetAction } from './MessageActionSheet';
import type { Message, WorkspaceMember, Channel } from '@/stores/workspace';
import { ReactionEmoji } from '@/shared/components/ReactionEmoji';
import RichTextDisplay from '@/components/RichTextDisplay';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { HuddleSystemMessage } from '@/features/huddle/components/HuddleSystemMessage';
import EmojiPicker from './EmojiPicker';

/// Reacting is the most common action in the product; a picker turns one tap
/// into three. Kept to three so the toolbar stays narrow enough not to cover
/// the message it belongs to.
const QUICK_REACTIONS = ['👍', '✅', '🎉'];
import MessageInput from './MessageInput';
import EditHistoryPanel from './EditHistoryPanel';
import { useCurrentWorkspaceRole } from '@/features/workspace/hooks/useCurrentWorkspaceRole';

interface ReactionGroup {
  emoji: string;
  count: number;
  hasOwn: boolean;
}

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
  onEdit: (messageId: string, content: string) => Promise<unknown> | void;
  onDelete: (messageId: string) => Promise<unknown> | void;
  onRetry?: (messageId: string, content: string) => void;
  onCopyLink?: (messageId: string) => void;
  onSave?: (message: Message) => void;
  onForward?: (message: Message) => void;
  dataQa?: string;
}

function groupReactions(message: Message, currentUserId: string): ReactionGroup[] {
  const groups: ReactionGroup[] = [];
  for (const r of message.reactions ?? []) {
    const existing = groups.find((g) => g.emoji === r.emoji);
    if (existing) {
      existing.count++;
      if (r.user_id === currentUserId) existing.hasOwn = true;
    } else {
      groups.push({ emoji: r.emoji, count: 1, hasOwn: r.user_id === currentUserId });
    }
  }
  return groups;
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
  onRetry,
  onCopyLink,
  onSave,
  onForward,
  dataQa = 'message-row',
}: MessageItemProps) {
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const reactBtnRef = useRef<HTMLButtonElement>(null);

  const reactionGroups = groupReactions(message, currentUserId);
  const isOwn = currentUserId === message.user_id;
  const isEdited = message.updated_at !== message.created_at;
  const [showHistory, setShowHistory] = useState(false);
  const rowRef = useRef<HTMLDivElement>(null);
  const [sheetOpen, setSheetOpen] = useState(false);
  const longPress = useLongPress(() => setSheetOpen(true), !editing && !confirmDelete);

  const sheetActions: SheetAction[] = [
    ...(onThreadOpen
      ? [
          {
            key: 'thread',
            label: 'Reply in thread',
            Icon: MessageSquare,
            onSelect: () => onThreadOpen(message),
          },
        ]
      : []),
    ...(onSave
      ? [{ key: 'save', label: 'Save message', Icon: Bookmark, onSelect: () => onSave(message) }]
      : []),
    ...(onCopyLink
      ? [{ key: 'copy-link', label: 'Copy link', Icon: Link2, onSelect: () => onCopyLink(message.id) }]
      : []),
    ...(onForward
      ? [{ key: 'forward', label: 'Forward', Icon: Forward, onSelect: () => onForward(message) }]
      : []),
    {
      key: 'pin',
      label: message.is_pinned ? 'Unpin from channel' : 'Pin to channel',
      Icon: Pin,
      onSelect: () => onTogglePin(message.id, message.is_pinned),
    },
    ...(isOwn
      ? [
          { key: 'edit', label: 'Edit message', Icon: Pencil, onSelect: () => setEditing(true) },
          {
            key: 'delete',
            label: 'Delete message',
            Icon: Trash2,
            onSelect: () => setConfirmDelete(true),
            destructive: true,
          },
        ]
      : []),
  ];

  useEffect(() => {
    if (!editing && !confirmDelete) return;
    rowRef.current?.scrollIntoView({ block: 'nearest' });
  }, [editing, confirmDelete]);
  const [showOverflow, setShowOverflow] = useState(false);
  const overflowAnchorRef = useRef<HTMLButtonElement>(null);
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

  const handleEditSave = async (content: string) => {
    await onEdit(message.id, content);
    setEditing(false);
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
    <div
      ref={rowRef}
      data-message-id={message.id}
      data-qa={dataQa}
      tabIndex={0}
      {...longPress}
      className={`group relative flex items-start gap-3 px-2 rounded-lg transition-colors hover:bg-surface/50 ${grouped ? 'py-0.5' : 'py-1.5'} ${message.pending ? 'opacity-50' : ''} ${isHighlighted ? 'bg-amber-500/10 ring-1 ring-inset ring-amber-500/25' : ''}`}
    >
      {grouped ? (
        <div className="w-8 shrink-0 flex justify-end pr-0.5">
          <span className="text-[10px] leading-5 text-muted opacity-0 group-hover:opacity-100 tabular-nums">
            {time}
          </span>
        </div>
      ) : (
        <Avatar userId={message.user_id} name={senderName} avatarUrl={senderAvatarUrl} className="mt-0.5" />
      )}
      <div className="flex-1 min-w-0">
        {!grouped && (
          <div className="flex items-baseline gap-2">
            <span className="text-sm font-semibold text-fg-soft">{senderName}</span>
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
                Bot
              </span>
            )}
            <span className="text-xs text-muted">{time}</span>
            {message.pending && <span className="text-xs text-muted italic">Sending…</span>}
            {editedMarker}
            {pinnedMarker}
          </div>
        )}

        {editing ? (
          <MessageInput
            key={`edit:${message.id}`}
            editing
            initialContent={message.content}
            members={members}
            channels={channels}
            onSend={handleEditSave}
            onCancel={() => setEditing(false)}
          />
        ) : (
          <>
            <RichTextDisplay content={message.content} />
            {grouped && (editedMarker || pinnedMarker) && (
              <div className="flex items-center gap-2">
                {editedMarker}
                {pinnedMarker}
              </div>
            )}
          </>
        )}

        {showHistory && (
          <EditHistoryPanel
            messageId={message.id}
            scope="channel"
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

        {reactionGroups.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-1">
            {reactionGroups.map((g) => (
              <button
                key={g.emoji}
                onClick={() => handleReactionToggle(g.emoji)}
                aria-pressed={g.hasOwn}
                data-qa="message-reaction"
                className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-xs border transition ${
                  g.hasOwn
                    ? 'bg-purple-600/20 border-purple-500/40 text-accent-soft'
                    : 'bg-raised/50 border-line-strong/50 text-fg-dim hover:bg-raised'
                }`}
              >
                <ReactionEmoji emoji={g.emoji} />
                <span>{g.count}</span>
              </button>
            ))}
          </div>
        )}

        {confirmDelete && (
          <div className="mt-2 flex items-center gap-2 text-xs">
            <span className="text-danger">Delete this message?</span>
            <button
              onClick={async () => {
                await onDelete(message.id);
                setConfirmDelete(false);
              }}
              data-qa="message-delete-confirm"
              className="px-2 py-1 bg-red-600 hover:bg-red-500 text-white rounded transition"
            >
              Delete
            </button>
            <button
              onClick={() => setConfirmDelete(false)}
              className="px-2 py-1 text-muted hover:text-fg transition"
            >
              Cancel
            </button>
          </div>
        )}
      </div>

      {!editing && !confirmDelete && (
        <div
          className={`message-actions absolute -top-3 right-2 flex items-center gap-0.5 bg-surface border border-line rounded-lg px-1 py-0.5 shadow-lg opacity-0 pointer-events-none transition-opacity group-hover:opacity-100 group-hover:pointer-events-auto group-focus-within:opacity-100 group-focus-within:pointer-events-auto ${
            showOverflow ? 'opacity-100 pointer-events-auto' : ''
          }`}
        >
          {/* Slack surfaces a few emoji inline because reacting is the most
              common action in the product, and a picker turns one tap into
              three. These are the three this instance uses most. */}
          {QUICK_REACTIONS.map((emoji) => (
            <button
              key={emoji}
              onClick={() => handleReactionToggle(emoji)}
              aria-label={`React with ${emoji}`}
              data-qa="message-quick-reaction"
              className="px-1 py-0.5 text-sm leading-none hover:bg-raised rounded transition"
            >
              {emoji}
            </button>
          ))}
          <div className="w-px h-4 bg-raised mx-0.5" />
          {onThreadOpen && (
            <button
              onClick={() => onThreadOpen(message)}
              aria-label="Reply in thread"
              data-qa="message-action-thread"
              className="p-1 text-muted hover:text-fg hover:bg-raised rounded transition"
            >
              <MessageSquare className="w-3.5 h-3.5" />
            </button>
          )}
          <div className="relative">
            <button
              ref={reactBtnRef}
              onClick={() => setShowEmojiPicker((v) => !v)}
              aria-label="Add reaction"
              data-qa="message-action-react"
              className="p-1 text-muted hover:text-fg hover:bg-raised rounded transition"
            >
              <SmilePlus className="w-3.5 h-3.5" />
            </button>
            {showEmojiPicker && (
              <EmojiPicker
                anchorRef={reactBtnRef}
                onSelect={(emoji) => {
                  handleReactionToggle(emoji);
                  setShowEmojiPicker(false);
                }}
                onClose={() => setShowEmojiPicker(false)}
              />
            )}
          </div>
          {onSave && (
            <button
              onClick={() => onSave(message)}
              aria-label="Save message"
              data-qa="message-action-save"
              className="p-1 text-muted hover:text-accent-soft hover:bg-raised rounded transition"
            >
              <Bookmark className="w-3.5 h-3.5" />
            </button>
          )}

          {/* The toolbar sits over the message, so it has to stay short. The
              four that get used live here; the rest are one click further. */}
          <div className="relative">
            <button
              ref={overflowAnchorRef}
              onClick={() => setShowOverflow((open) => !open)}
              aria-label="More actions"
              aria-expanded={showOverflow}
              data-qa="message-action-more"
              className="p-1 text-muted hover:text-fg hover:bg-raised rounded transition"
            >
              <MoreHorizontal className="w-3.5 h-3.5" />
            </button>
            {showOverflow && (
              <AnchoredPopover
                anchorRef={overflowAnchorRef}
                onClose={() => setShowOverflow(false)}
                dataQa="message-overflow"
                className="w-44 bg-surface border border-line rounded-lg shadow-xl py-1"
              >
                <button
                  onClick={() => {
                    onTogglePin(message.id, message.is_pinned);
                    setShowOverflow(false);
                  }}
                  data-qa="message-action-pin"
                  className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2"
                >
                  <Pin className="w-3.5 h-3.5" /> {message.is_pinned ? 'Unpin' : 'Pin'}
                </button>
                {onCopyLink && (
                  <button
                    onClick={() => {
                      onCopyLink(message.id);
                      setShowOverflow(false);
                    }}
                    data-qa="message-action-copy-link"
                    className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2"
                  >
                    <Link2 className="w-3.5 h-3.5" /> Copy link
                  </button>
                )}
                {onForward && (
                  <button
                    onClick={() => {
                      onForward(message);
                      setShowOverflow(false);
                    }}
                    data-qa="message-action-forward"
                    className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2"
                  >
                    <Forward className="w-3.5 h-3.5" /> Forward
                  </button>
                )}
                {isOwn && (
                  <button
                    onClick={() => {
                      setEditing(true);
                      setShowOverflow(false);
                    }}
                    data-qa="message-action-edit"
                    className="w-full px-3 py-1.5 text-left text-sm text-fg-dim hover:bg-raised flex items-center gap-2"
                  >
                    <Pencil className="w-3.5 h-3.5" /> Edit
                  </button>
                )}
                {isOwn && (
                  <button
                    onClick={() => {
                      setConfirmDelete(true);
                      setShowOverflow(false);
                    }}
                    data-qa="message-action-delete"
                    className="w-full px-3 py-1.5 text-left text-sm text-danger hover:bg-raised flex items-center gap-2"
                  >
                    <Trash2 className="w-3.5 h-3.5" /> Delete
                  </button>
                )}
              </AnchoredPopover>
            )}
          </div>
        </div>
      )}

      {sheetOpen && (
        <MessageActionSheet
          quickReactions={QUICK_REACTIONS}
          onReact={(emoji) => handleReactionToggle(emoji)}
          actions={sheetActions}
          onClose={() => setSheetOpen(false)}
        />
      )}
    </div>
  );
}

export default memo(MessageItem);
