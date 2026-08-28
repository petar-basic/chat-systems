import { useEffect, useRef } from 'react';
import { X, MessageSquare } from 'lucide-react';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import {
  useConversationThread,
  useSendConversationThreadReply,
  useConversationThreadReplyActions,
  type ConversationMessage,
} from '@/hooks/queries/useConversations';
import { useWorkspaceStore } from '@/stores/workspace';
import RichTextDisplay from './RichTextDisplay';
import { MessageInput } from '@/features/messaging';
import { ConversationMessageRow } from '@/features/messaging/ConversationView';

interface Props {
  conversationId: string;
  parentMessage: ConversationMessage;
  instanceUrl?: string;
  onClose: () => void;
}

export default function ConversationThreadPanel({
  conversationId,
  parentMessage,
  instanceUrl,
  onClose,
}: Props) {
  const { data: replies = [], isLoading } = useConversationThread(parentMessage.id, instanceUrl);
  const sendReply = useSendConversationThreadReply(conversationId, parentMessage.id, instanceUrl);
  const currentUserId = useWorkspaceStore((s) => s.currentUserId) ?? '';
  const { edit, remove, toggleReaction } = useConversationThreadReplyActions(
    parentMessage.id,
    currentUserId,
    instanceUrl,
  );
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [replies]);

  const { getUser } = useUserCache();
  const parentSender = getUser(parentMessage.user_id);
  const parentName = displayNameOf(parentSender?.display_name);

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 flex flex-col border-l border-line/50 bg-app/80"
      data-qa="dm-thread-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-line/50 shrink-0">
        <h3 className="text-sm font-bold text-fg flex items-center gap-2">
          <MessageSquare className="w-4 h-4" />
          Thread
        </h3>
        <button
          onClick={onClose}
          aria-label="Close thread"
          className="text-muted hover:text-fg transition cursor-pointer"
        >
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
        <div className="mt-2 text-xs text-muted" data-qa="dm-thread-count">
          {replies.length} {replies.length === 1 ? 'reply' : 'replies'}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-2">
        {isLoading ? (
          <div className="flex justify-center py-4">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : replies.length === 0 ? (
          <div className="text-center py-8 text-muted text-sm">No replies yet.</div>
        ) : (
          replies.map((reply) => (
            <ConversationMessageRow
              key={reply.id}
              msg={reply}
              isOwn={reply.user_id === currentUserId}
              currentUserId={currentUserId}
              onEdit={(content) => edit(reply.id, content)}
              onDelete={() => remove(reply.id)}
              onToggleReaction={(emoji, hasOwn) => toggleReaction(reply.id, emoji, hasOwn)}
            />
          ))
        )}
        <div ref={endRef} />
      </div>

      <div className="border-t border-line/50">
        <MessageInput
          key={`dm-thread:${parentMessage.id}`}
          isDm
          placeholder="Reply…"
          draftKey={`dm-thread:${parentMessage.id}`}
          onSend={async (content) => {
            await sendReply.mutateAsync(content);
          }}
        />
      </div>
    </div>
  );
}
