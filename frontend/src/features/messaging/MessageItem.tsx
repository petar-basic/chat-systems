import { memo, useState, useRef } from 'react';
import { Pencil, Trash2, MessageSquare, SmilePlus, Pin, Link2 } from 'lucide-react';
import type { Message, WorkspaceMember, Channel } from '@/stores/workspace';
import RichTextDisplay from '@/components/RichTextDisplay';
import { HuddleSystemMessage } from '@/features/huddle/components/HuddleSystemMessage';
import EmojiPicker from './EmojiPicker';
import MessageInput from './MessageInput';

interface ReactionGroup {
  emoji: string;
  count: number;
  hasOwn: boolean;
}

interface MessageItemProps {
  message: Message;
  currentUserId: string;
  senderName: string;
  avatarColor: string;
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
  avatarColor,
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
}: MessageItemProps) {
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const reactBtnRef = useRef<HTMLButtonElement>(null);

  const reactionGroups = groupReactions(message, currentUserId);
  const isOwn = currentUserId === message.user_id;
  const isEdited = message.updated_at !== message.created_at;

  if (message.metadata?.kind === 'huddle_started' && message.metadata.huddle_id && !message.deleted_at) {
    return (
      <HuddleSystemMessage
        channelId={message.channel_id}
        huddleId={message.metadata.huddle_id}
        initiatorId={message.metadata.initiator_id ?? message.user_id}
      />
    );
  }
  const initials = senderName.charAt(0).toUpperCase();
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
        <div className="w-8 h-8 rounded-full bg-slate-700 flex items-center justify-center text-sm shrink-0 mt-0.5">
          <Trash2 className="w-3.5 h-3.5 text-slate-400" />
        </div>
        <div className="flex-1 min-w-0 py-1">
          <p className="text-sm text-slate-400 italic">This message was deleted</p>
        </div>
      </div>
    );
  }

  return (
    <div
      data-message-id={message.id}
      data-qa="message-row"
      tabIndex={0}
      className={`group relative flex items-start gap-3 px-2 rounded-lg transition-colors hover:bg-slate-800/50 ${grouped ? 'py-0.5' : 'py-1.5'} ${message.pending ? 'opacity-50' : ''} ${isHighlighted ? 'bg-amber-500/10 ring-1 ring-inset ring-amber-500/25' : ''}`}
    >
      {grouped ? (
        <div className="w-8 shrink-0 flex justify-end pr-0.5">
          <span className="text-[10px] leading-5 text-slate-400 opacity-0 group-hover:opacity-100 tabular-nums">
            {time}
          </span>
        </div>
      ) : (
        <div
          className={`w-8 h-8 rounded-full ${avatarColor} flex items-center justify-center text-sm font-bold shrink-0 mt-0.5`}
        >
          {initials}
        </div>
      )}
      <div className="flex-1 min-w-0">
        {!grouped && (
          <div className="flex items-baseline gap-2">
            <span className="text-sm font-semibold text-slate-200">{senderName}</span>
            <span className="text-xs text-slate-400">{time}</span>
            {message.pending && <span className="text-xs text-slate-400 italic">Sending…</span>}
            {isEdited && !message.pending && <span className="text-xs text-slate-400 italic">(edited)</span>}
            {message.is_pinned && (
              <span className="text-xs text-amber-400 flex items-center gap-0.5">
                <Pin className="w-3 h-3" /> pinned
              </span>
            )}
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
          <RichTextDisplay content={message.content} />
        )}

        {message.failed && (
          <div className="mt-1 flex items-center gap-2 text-xs text-red-400" data-qa="message-failed">
            <span>Failed to send.</span>
            {onRetry && (
              <button
                onClick={() => onRetry(message.id, message.content)}
                data-qa="message-retry"
                className="font-semibold text-red-300 hover:text-red-200 underline"
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
            className="mt-1 flex items-center gap-1.5 text-xs text-purple-400 hover:text-purple-300 transition group/thread"
          >
            <MessageSquare className="w-3.5 h-3.5" />
            <span className="font-medium">
              {message.reply_count} {message.reply_count === 1 ? 'reply' : 'replies'}
            </span>
            <span className="text-slate-400 group-hover/thread:text-purple-400 transition">View thread</span>
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
                    ? 'bg-purple-600/20 border-purple-500/40 text-purple-300'
                    : 'bg-slate-700/50 border-slate-600/50 text-slate-300 hover:bg-slate-700'
                }`}
              >
                <span>{g.emoji}</span>
                <span>{g.count}</span>
              </button>
            ))}
          </div>
        )}

        {confirmDelete && (
          <div className="mt-2 flex items-center gap-2 text-xs">
            <span className="text-red-400">Delete this message?</span>
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
              className="px-2 py-1 text-slate-400 hover:text-white transition"
            >
              Cancel
            </button>
          </div>
        )}
      </div>

      {!editing && !confirmDelete && (
        <div className="absolute -top-3 right-2 flex items-center gap-0.5 bg-slate-800 border border-slate-700 rounded-lg px-1 py-0.5 shadow-lg opacity-0 pointer-events-none transition-opacity group-hover:opacity-100 group-hover:pointer-events-auto group-focus-within:opacity-100 group-focus-within:pointer-events-auto">
          {onThreadOpen && (
            <button
              onClick={() => onThreadOpen(message)}
              aria-label="Reply in thread"
              data-qa="message-action-thread"
              className="p-1 text-slate-400 hover:text-white hover:bg-slate-700 rounded transition"
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
              className="p-1 text-slate-400 hover:text-white hover:bg-slate-700 rounded transition"
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
          <button
            onClick={() => onTogglePin(message.id, message.is_pinned)}
            aria-label={message.is_pinned ? 'Unpin message' : 'Pin message'}
            data-qa="message-action-pin"
            className="p-1 text-slate-400 hover:text-amber-400 hover:bg-slate-700 rounded transition"
          >
            <Pin className="w-3.5 h-3.5" />
          </button>
          {onCopyLink && (
            <button
              onClick={() => onCopyLink(message.id)}
              aria-label="Copy link to message"
              data-qa="message-action-copy-link"
              className="p-1 text-slate-400 hover:text-white hover:bg-slate-700 rounded transition"
            >
              <Link2 className="w-3.5 h-3.5" />
            </button>
          )}
          {isOwn && (
            <button
              onClick={() => setEditing(true)}
              aria-label="Edit message"
              data-qa="message-action-edit"
              className="p-1 text-slate-400 hover:text-white hover:bg-slate-700 rounded transition"
            >
              <Pencil className="w-3.5 h-3.5" />
            </button>
          )}
          {isOwn && (
            <button
              onClick={() => setConfirmDelete(true)}
              aria-label="Delete message"
              data-qa="message-action-delete"
              className="p-1 text-slate-400 hover:text-red-400 hover:bg-slate-700 rounded transition"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default memo(MessageItem);
