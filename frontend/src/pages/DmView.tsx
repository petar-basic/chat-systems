import { useEffect, useRef, useState } from 'react';
import { useUserCache } from '../stores/users';
import { usePresenceStore } from '../stores/presence';
import { Circle, ArrowLeft, Pencil, Trash2, SmilePlus, Menu } from 'lucide-react';
import {
  useDirectMessages,
  useSendDirectMessage,
  useEditDirectMessage,
  useDeleteDirectMessage,
  useReactToDm,
  useRemoveDmReaction,
} from '../hooks/queries/useDm';
import { MessageInput, EmojiPicker } from '@/features/messaging';
import RichTextDisplay from '../components/RichTextDisplay';
import type { DirectMessage } from '../hooks/queries/useDm';
import { displayNameOf, avatarColorFor } from '@/lib/userHelpers';
import { ConnectionBanner } from '@/shared/components/ConnectionBanner/ConnectionBanner';
import { QueryState } from '@/shared/components/QueryState/QueryState';
import { EmptyLabels, MESSAGE_GROUP_WINDOW_MS } from '@/shared/constants';

interface Props {
  workspaceId: string;
  instanceUrl: string;
  partnerId: string;
  currentUserId: string;
  onClose: () => void;
  onOpenNav?: () => void;
}

export default function DmView({
  workspaceId,
  instanceUrl,
  partnerId,
  currentUserId,
  onClose,
  onOpenNav,
}: Props) {
  const { getUser } = useUserCache();
  const partner = getUser(partnerId);
  const status = usePresenceStore((s) => s.getStatus(partnerId));
  const dotColor =
    status === 'online' ? 'text-green-500' : status === 'away' ? 'text-amber-500' : 'text-slate-600';
  const partnerName = displayNameOf(partner?.display_name);

  const { data, isLoading, isError, refetch, fetchNextPage, hasNextPage, isFetchingNextPage } =
    useDirectMessages(workspaceId, partnerId, instanceUrl);

  const sendMutation = useSendDirectMessage(workspaceId, partnerId, currentUserId, instanceUrl);
  const editMutation = useEditDirectMessage(workspaceId, partnerId, instanceUrl);
  const deleteMutation = useDeleteDirectMessage(workspaceId, partnerId, instanceUrl);
  const reactMutation = useReactToDm(workspaceId, partnerId, currentUserId, instanceUrl);
  const removeReactionMutation = useRemoveDmReaction(workspaceId, partnerId, currentUserId, instanceUrl);

  const toggleReaction = (messageId: string, emoji: string, hasOwn: boolean) => {
    if (hasOwn) removeReactionMutation.mutate({ messageId, emoji });
    else reactMutation.mutate({ messageId, emoji });
  };
  const bottomRef = useRef<HTMLDivElement>(null);
  const [hasScrolledUp, setHasScrolledUp] = useState(false);

  const messages = data?.pages.flatMap((p) => p.data) ?? [];
  const displayMessages = [...messages].reverse();

  useEffect(() => {
    if (!hasScrolledUp) {
      bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages.length, hasScrolledUp]);

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    const atTop = el.scrollTop < 100;
    if (atTop && hasNextPage && !isFetchingNextPage) {
      fetchNextPage();
    }
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 100;
    setHasScrolledUp(!atBottom);
  };

  const handleSend = async (content: string) => {
    sendMutation.mutate({ content, id: crypto.randomUUID() });
    setHasScrolledUp(false);
  };

  const messageCount = displayMessages.length;

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
        <Circle className={`w-2.5 h-2.5 fill-current ${dotColor} shrink-0`} />
        <span className="font-semibold text-white">{partnerName}</span>
        {status === 'online' && <span className="text-xs text-slate-400">Active now</span>}
      </div>

      <QueryState
        isLoading={isLoading}
        isError={isError}
        isEmpty={messageCount === 0}
        onRetry={() => void refetch()}
        empty={<p className="text-sm">{EmptyLabels.DmBeginning(partnerName)}</p>}
      >
        <div className="flex-1 overflow-y-auto px-4 py-4 flex flex-col" onScroll={handleScroll}>
          {isFetchingNextPage && (
            <div className="text-center text-xs text-slate-400 py-2">Loading older messages...</div>
          )}
          {displayMessages.map((msg, i) => {
            const prev = displayMessages[i - 1];
            const grouped =
              !!prev &&
              !prev.deleted_at &&
              prev.from_user_id === msg.from_user_id &&
              new Date(msg.created_at).toDateString() === new Date(prev.created_at).toDateString() &&
              new Date(msg.created_at).getTime() - new Date(prev.created_at).getTime() <
                MESSAGE_GROUP_WINDOW_MS;
            return (
              <DmMessage
                key={msg.id}
                msg={msg}
                grouped={grouped}
                isOwn={msg.from_user_id === currentUserId}
                currentUserId={currentUserId}
                onEdit={(content) => editMutation.mutateAsync({ messageId: msg.id, content })}
                onDelete={() => deleteMutation.mutateAsync({ messageId: msg.id })}
                onToggleReaction={(emoji, hasOwn) => toggleReaction(msg.id, emoji, hasOwn)}
              />
            );
          })}
          <div ref={bottomRef} />
        </div>
      </QueryState>

      <MessageInput
        key={`dm:${partnerId}`}
        channelName={partnerName}
        draftKey={`dm:${partnerId}`}
        isDm
        onSend={handleSend}
      />
    </div>
  );
}

interface DmMessageProps {
  msg: DirectMessage;
  grouped?: boolean;
  isOwn: boolean;
  currentUserId: string;
  onEdit: (content: string) => Promise<unknown>;
  onDelete: () => Promise<unknown>;
  onToggleReaction: (emoji: string, hasOwn: boolean) => void;
}

function DmMessage({
  msg,
  grouped,
  isOwn,
  currentUserId,
  onEdit,
  onDelete,
  onToggleReaction,
}: DmMessageProps) {
  const { getUser } = useUserCache();
  const sender = getUser(msg.from_user_id);
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
      <div className="flex items-start gap-3 py-1.5 px-2 rounded-lg opacity-50" data-qa="dm-deleted">
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
      data-qa="dm-message"
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
        <div
          className={`w-8 h-8 rounded-full ${avatarColorFor(msg.from_user_id)} flex items-center justify-center text-sm font-bold shrink-0 mt-0.5`}
        >
          {senderName.charAt(0).toUpperCase()}
        </div>
      )}
      <div className="flex-1 min-w-0">
        {!grouped && (
          <div className="flex items-baseline gap-2">
            <span className="text-sm font-semibold text-slate-200">{senderName}</span>
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
                <span>{g.emoji}</span>
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
