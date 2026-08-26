import { useCallback, useMemo, useRef, useState } from 'react';
import { useUserCache } from '@/stores/users';
import { usePresenceStore } from '@/stores/presence';
import { ArrowLeft, Pencil, Trash2, SmilePlus, Menu, MessageSquare, Bookmark, Forward } from 'lucide-react';
import {
  useConversationMessages,
  useSendConversationMessage,
  useEditConversationMessage,
  useDeleteConversationMessage,
  useReactToConversationMessage,
  useRemoveConversationReaction,
} from '@/hooks/queries/useConversations';
import { MessageInput, EmojiPicker } from '@/features/messaging';
import { ReactionEmoji } from '@/shared/components/ReactionEmoji';
import RichTextDisplay from '@/components/RichTextDisplay';
import PresenceDot from '@/components/PresenceDot';
import type { ConversationMessage } from '@/hooks/queries/useConversations';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { ConnectionBanner } from '@/shared/components/ConnectionBanner/ConnectionBanner';
import { QueryState } from '@/shared/components/QueryState/QueryState';
import { HuddleStartButton } from '@/features/huddle';
import { EmptyLabels } from '@/shared/constants';
import VirtualMessageList from './VirtualMessageList';
import { DaySeparator } from './MessageList';
import { buildMessageRows, type MessageRow } from './messageRows';

interface Props {
  workspaceId: string;
  instanceUrl: string;
  conversationId: string;
  title: string;
  participantIds: string[];
  kind: 'direct' | 'group';
  currentUserId: string;
  onClose: () => void;
  onOpenNav?: () => void;
  onOpenThread: (message: ConversationMessage) => void;
  onSave: (message: ConversationMessage) => void;
  onForward: (message: ConversationMessage) => void;
}

export default function ConversationView({
  workspaceId,
  instanceUrl,
  conversationId,
  title,
  participantIds,
  kind,
  currentUserId,
  onClose,
  onOpenNav,
  onOpenThread,
  onSave,
  onForward,
}: Props) {
  const partnerId = participantIds.find((id) => id !== currentUserId) ?? currentUserId;
  const { getUser } = useUserCache();
  const partner = getUser(partnerId);
  const status = usePresenceStore((s) => s.getStatus(partnerId));

  const { data, isLoading, isError, refetch, fetchNextPage, hasNextPage, isFetchingNextPage } =
    useConversationMessages(conversationId, instanceUrl);

  const sendMutation = useSendConversationMessage(workspaceId, conversationId, currentUserId, instanceUrl);
  const editMutation = useEditConversationMessage(conversationId, instanceUrl);
  const deleteMutation = useDeleteConversationMessage(conversationId, instanceUrl);
  const reactMutation = useReactToConversationMessage(conversationId, currentUserId, instanceUrl);
  const removeReactionMutation = useRemoveConversationReaction(conversationId, currentUserId, instanceUrl);

  const toggleReaction = useCallback(
    (messageId: string, emoji: string, hasOwn: boolean) => {
      if (hasOwn) removeReactionMutation.mutate({ messageId, emoji });
      else reactMutation.mutate({ messageId, emoji });
    },
    [reactMutation, removeReactionMutation],
  );
  const displayMessages = useMemo(() => [...(data?.pages.flatMap((p) => p.data) ?? [])].reverse(), [data]);
  const rows = useMemo(() => buildMessageRows(displayMessages), [displayMessages]);

  const handleSend = async (content: string) => {
    sendMutation.mutate({ content, id: crypto.randomUUID() });
  };

  const messageCount = displayMessages.length;

  const renderRow = useCallback(
    (row: MessageRow<ConversationMessage>) => {
      if (row.kind === 'day') return <DaySeparator at={row.at} />;
      const msg = row.message;
      return (
        <ConversationMessageRow
          msg={msg}
          grouped={row.grouped}
          isOwn={msg.user_id === currentUserId}
          currentUserId={currentUserId}
          onEdit={(content) => editMutation.mutateAsync({ messageId: msg.id, content })}
          onDelete={() => deleteMutation.mutateAsync({ messageId: msg.id })}
          onToggleReaction={(emoji, hasOwn) => toggleReaction(msg.id, emoji, hasOwn)}
          onOpenThread={() => onOpenThread(msg)}
          onSave={() => onSave(msg)}
          onForward={() => onForward(msg)}
        />
      );
    },
    [currentUserId, deleteMutation, editMutation, onForward, onOpenThread, onSave, toggleReaction],
  );

  return (
    <div role="main" aria-label="Direct message" className="flex-1 flex flex-col min-w-0">
      <ConnectionBanner instanceUrl={instanceUrl} />
      <div className="h-12 px-4 flex items-center gap-3 border-b border-slate-700/50 shrink-0">
        {onOpenNav && (
          <button
            onClick={onOpenNav}
            aria-label="Open navigation"
            className="lg:hidden p-1.5 -ml-1 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700/50"
          >
            <Menu className="w-5 h-5" />
          </button>
        )}
        <button
          onClick={onClose}
          aria-label="Back to channels"
          className="text-slate-400 hover:text-white transition cursor-pointer mr-1"
        >
          <ArrowLeft className="w-4 h-4" />
        </button>
        {kind === 'direct' ? (
          <div className="relative shrink-0">
            <Avatar userId={partnerId} name={title} avatarUrl={partner?.avatar_url} size="sm" />
            <PresenceDot
              userId={partnerId}
              className="absolute -bottom-0.5 -right-0.5 ring-2 ring-slate-900"
            />
          </div>
        ) : (
          <div className="flex -space-x-2 shrink-0" data-qa="conversation-participants">
            {participantIds.slice(0, 3).map((id) => (
              <Avatar
                key={id}
                userId={id}
                name={displayNameOf(getUser(id)?.display_name)}
                avatarUrl={getUser(id)?.avatar_url}
                size="xs"
                className="ring-2 ring-slate-900"
              />
            ))}
          </div>
        )}
        <span className="font-semibold text-white truncate" data-qa="conversation-title">
          {title}
        </span>
        {kind === 'group' && (
          <span className="text-xs text-slate-400 shrink-0">{participantIds.length} people</span>
        )}
        {kind === 'direct' && status === 'online' && (
          <span className="text-xs text-slate-400">Active now</span>
        )}
        {kind === 'direct' && (
          <div className="ml-auto">
            <HuddleStartButton
              workspaceId={workspaceId}
              instanceUrl={instanceUrl}
              partnerId={partnerId}
              currentUserId={currentUserId}
            />
          </div>
        )}
      </div>

      <QueryState
        isLoading={isLoading}
        isError={isError}
        isEmpty={messageCount === 0}
        onRetry={() => void refetch()}
        empty={<p className="text-sm">{EmptyLabels.DmBeginning(title)}</p>}
      >
        <VirtualMessageList
          rows={rows}
          renderRow={renderRow}
          hasOlder={!!hasNextPage}
          isLoadingOlder={isFetchingNextPage}
          onLoadOlder={fetchNextPage}
          qa="conversation-message-list"
          ariaLabel="Direct messages"
        />
      </QueryState>

      <MessageInput
        key={`conversation:${conversationId}`}
        channelName={title}
        draftKey={`conversation:${conversationId}`}
        isDm
        onSend={handleSend}
        scheduleTarget={{ conversationId }}
        workspaceId={workspaceId}
        instanceUrl={instanceUrl}
      />
    </div>
  );
}

interface ConversationMessageProps {
  msg: ConversationMessage;
  grouped?: boolean;
  isOwn: boolean;
  currentUserId: string;
  onEdit: (content: string) => Promise<unknown>;
  onDelete: () => Promise<unknown>;
  onToggleReaction: (emoji: string, hasOwn: boolean) => void;
  onOpenThread: () => void;
  onSave: () => void;
  onForward: () => void;
}

function ConversationMessageRow({
  msg,
  grouped,
  isOwn,
  currentUserId,
  onEdit,
  onDelete,
  onToggleReaction,
  onOpenThread,
  onSave,
  onForward,
}: ConversationMessageProps) {
  const { getUser } = useUserCache();
  const sender = getUser(msg.user_id);
  const senderName = displayNameOf(sender?.display_name);

  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const reactBtnRef = useRef<HTMLButtonElement>(null);

  const time = new Date(msg.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

  const reactionGroups: { emoji: string; count: number; hasOwn: boolean }[] = [];
  for (const r of msg.reactions ?? []) {
    const g = reactionGroups.find((x) => x.emoji === r.emoji);
    if (g) {
      g.count++;
      if (r.user_id === currentUserId) g.hasOwn = true;
    } else {
      reactionGroups.push({ emoji: r.emoji, count: 1, hasOwn: r.user_id === currentUserId });
    }
  }

  const handleReactionToggle = (emoji: string) => {
    const hasOwn = (msg.reactions ?? []).some((r) => r.emoji === emoji && r.user_id === currentUserId);
    onToggleReaction(emoji, hasOwn);
  };

  const handleEditSave = async (content: string) => {
    const trimmed = content.trim();
    if (!trimmed) return;
    await onEdit(trimmed);
    setEditing(false);
  };

  if (msg.deleted_at) {
    return (
      <div
        className="flex items-start gap-3 py-1.5 px-2 rounded-lg opacity-50"
        data-qa="conversation-message-deleted"
      >
        <div className="w-8 h-8 rounded-full bg-slate-700 flex items-center justify-center shrink-0 mt-0.5">
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
      data-qa="conversation-message"
      tabIndex={0}
      className={`group relative flex items-start gap-3 px-2 rounded-lg transition-colors hover:bg-slate-800/50 ${grouped ? 'py-0.5' : 'py-1.5'} ${msg.pending ? 'opacity-50' : ''}`}
    >
      {grouped ? (
        <div className="w-8 shrink-0 flex justify-end pr-0.5">
          <span className="text-[10px] leading-5 text-slate-400 opacity-0 group-hover:opacity-100 tabular-nums">
            {time}
          </span>
        </div>
      ) : (
        <Avatar userId={msg.user_id} name={senderName} avatarUrl={sender?.avatar_url} className="mt-0.5" />
      )}
      <div className="flex-1 min-w-0">
        {!grouped && (
          <div className="flex items-baseline gap-2">
            <span className="text-sm font-semibold text-slate-200">{senderName}</span>
            {sender?.status_emoji && (
              <span data-qa="message-status-emoji" title={sender.status_text ?? undefined}>
                {sender.status_emoji}
              </span>
            )}
            <span className="text-xs text-slate-400">{time}</span>
            {msg.edited_at && <span className="text-xs text-slate-400 italic">(edited)</span>}
            {msg.pending && <span className="text-xs text-slate-400 italic">Sending…</span>}
          </div>
        )}

        {editing ? (
          <MessageInput
            key={`edit:${msg.id}`}
            editing
            isDm
            initialContent={msg.content}
            onSend={handleEditSave}
            onCancel={() => setEditing(false)}
          />
        ) : (
          <RichTextDisplay content={msg.content} />
        )}

        {msg.reply_count > 0 && (
          <button
            onClick={onOpenThread}
            data-qa="dm-thread-open"
            className="mt-1 inline-flex items-center gap-1.5 px-1.5 py-0.5 rounded text-xs text-purple-300 hover:bg-slate-700/50 transition cursor-pointer"
          >
            <MessageSquare className="w-3 h-3" />
            {msg.reply_count} {msg.reply_count === 1 ? 'reply' : 'replies'}
          </button>
        )}

        {reactionGroups.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-1">
            {reactionGroups.map((g) => (
              <button
                key={g.emoji}
                onClick={() => handleReactionToggle(g.emoji)}
                aria-pressed={g.hasOwn}
                data-qa="dm-reaction"
                className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-xs border transition ${
                  g.hasOwn
                    ? 'bg-purple-600/20 border-purple-500/40 text-purple-300'
                    : 'bg-slate-700/50 border-slate-600/50 text-slate-300 hover:bg-slate-700'
                }`}
              >
                <ReactionEmoji emoji={g.emoji} />
                <span>{g.count}</span>
              </button>
            ))}
          </div>
        )}

        {confirmDelete && (
          <div className="mt-1 flex items-center gap-2 text-xs">
            <span className="text-red-400">Delete this message?</span>
            <button
              onClick={async () => {
                await onDelete();
                setConfirmDelete(false);
              }}
              className="px-2 py-1 bg-red-600 hover:bg-red-500 text-white rounded"
            >
              Delete
            </button>
            <button
              onClick={() => setConfirmDelete(false)}
              className="px-2 py-1 text-slate-400 hover:text-white"
            >
              Cancel
            </button>
          </div>
        )}
      </div>

      {!editing && !confirmDelete && (
        <div className="absolute -top-3 right-2 flex items-center gap-0.5 bg-slate-800 border border-slate-700 rounded-lg px-1 py-0.5 shadow-lg opacity-0 pointer-events-none transition-opacity group-hover:opacity-100 group-hover:pointer-events-auto group-focus-within:opacity-100 group-focus-within:pointer-events-auto">
          <div className="relative">
            <button
              ref={reactBtnRef}
              onClick={() => setShowEmojiPicker((v) => !v)}
              aria-label="Add reaction"
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
            onClick={onOpenThread}
            aria-label="Reply in thread"
            data-qa="dm-action-thread"
            className="p-1 text-slate-400 hover:text-white hover:bg-slate-700 rounded transition"
          >
            <MessageSquare className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={onSave}
            aria-label="Save message"
            data-qa="dm-action-save"
            className="p-1 text-slate-400 hover:text-purple-300 hover:bg-slate-700 rounded transition"
          >
            <Bookmark className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={onForward}
            aria-label="Forward message"
            data-qa="dm-action-forward"
            className="p-1 text-slate-400 hover:text-white hover:bg-slate-700 rounded transition"
          >
            <Forward className="w-3.5 h-3.5" />
          </button>
          {isOwn && (
            <button
              onClick={() => setEditing(true)}
              aria-label="Edit message"
              className="p-1 text-slate-400 hover:text-white hover:bg-slate-700 rounded transition"
            >
              <Pencil className="w-3.5 h-3.5" />
            </button>
          )}
          {isOwn && (
            <button
              onClick={() => setConfirmDelete(true)}
              aria-label="Delete message"
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
