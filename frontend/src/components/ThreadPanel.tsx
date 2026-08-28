import { useEffect, useRef } from 'react';
import { X, MessageSquare } from 'lucide-react';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import type { Message, WorkspaceMember, Channel } from '@/stores/workspace';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useThreadMessages, useSendThreadReply, useThreadReplyActions } from '@/hooks/queries/useThreads';
import RichTextDisplay from './RichTextDisplay';
import { MessageInput, MessageItem } from '@/features/messaging';

interface Props {
  parentMessage: Message;
  members?: WorkspaceMember[];
  channels?: Channel[];
  onClose: () => void;
}

export default function ThreadPanel({ parentMessage, members, channels, onClose }: Props) {
  const { data: replies = [], isLoading: loading } = useThreadMessages(parentMessage.id);
  const sendReply = useSendThreadReply(parentMessage.id, parentMessage.channel_id);
  const currentUserId = useWorkspaceStore((s) => s.currentUserId) ?? '';
  const { toggleReaction, edit, remove, togglePin } = useThreadReplyActions(parentMessage.id, currentUserId);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [replies]);

  const handleSend = async (content: string) => {
    await sendReply.mutateAsync(content);
  };

  const { getUser } = useUserCache();
  const parentSender = getUser(parentMessage.user_id);
  const parentName = displayNameOf(parentSender?.display_name);

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 flex flex-col border-l border-line/50 bg-app/80"
      data-qa="thread-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-line/50 shrink-0">
        <h3 className="text-sm font-bold text-fg flex items-center gap-2">
          <MessageSquare className="w-4 h-4" />
          Thread
        </h3>
        <button onClick={onClose} aria-label="Close thread" className="text-muted hover:text-fg transition">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="px-4 py-3 border-b border-line/30">
        <div className="flex items-start gap-2.5">
          <Avatar userId={parentMessage.user_id} name={parentName} avatarUrl={parentSender?.avatar_url} />
          <div className="flex-1 min-w-0">
            <div className="flex items-baseline gap-2">
              <span className="text-sm font-semibold text-fg-soft">{parentName}</span>
              <span className="text-xs text-muted">
                {new Date(parentMessage.created_at).toLocaleTimeString([], {
                  hour: '2-digit',
                  minute: '2-digit',
                })}
              </span>
            </div>
            <RichTextDisplay content={parentMessage.content} />
          </div>
        </div>
        <div className="mt-2 text-xs text-muted">
          {replies.length} {replies.length === 1 ? 'reply' : 'replies'}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-2">
        {loading ? (
          <div className="flex justify-center py-4">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : replies.length === 0 ? (
          <div className="text-center py-8 text-muted text-sm">No replies yet. Start the conversation!</div>
        ) : (
          replies.map((r) => {
            const sender = getUser(r.user_id);
            return (
              <MessageItem
                key={r.id}
                message={r}
                currentUserId={currentUserId}
                senderName={displayNameOf(sender?.display_name)}
                senderAvatarUrl={sender?.avatar_url}
                members={members}
                channels={channels}
                onToggleReaction={toggleReaction}
                onTogglePin={togglePin}
                onEdit={edit}
                onDelete={remove}
              />
            );
          })
        )}
        <div ref={endRef} />
      </div>

      <div className="border-t border-line/50">
        <MessageInput
          key={`thread:${parentMessage.id}`}
          channelName=""
          placeholder="Reply…"
          members={members}
          channels={channels}
          draftKey={`thread:${parentMessage.id}`}
          onSend={handleSend}
        />
      </div>
    </div>
  );
}
