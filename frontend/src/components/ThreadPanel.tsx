import { useEffect, useRef } from 'react';
import { X, MessageSquare } from 'lucide-react';
import { useUserCache } from '@/stores/users';
import type { Message, WorkspaceMember, Channel } from '@/stores/workspace';
import { avatarColorFor, displayNameOf } from '@/lib/userHelpers';
import { useThreadMessages, useSendThreadReply } from '@/hooks/queries/useThreads';
import RichTextDisplay from './RichTextDisplay';
import { MessageInput } from '@/features/messaging';

interface Props {
  parentMessage: Message;
  members?: WorkspaceMember[];
  channels?: Channel[];
  onClose: () => void;
}

function ThreadMessage({ message }: { message: Message }) {
  const { getUser } = useUserCache();
  const displayName = displayNameOf(getUser(message.user_id)?.display_name);

  return (
    <div className="flex items-start gap-2.5 py-1.5">
      <div
        className={`w-7 h-7 rounded-full ${avatarColorFor(message.user_id)} flex items-center justify-center text-xs font-bold shrink-0 mt-0.5`}
      >
        {displayName.charAt(0).toUpperCase()}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-semibold text-slate-200">{displayName}</span>
          <span className="text-xs text-slate-400">
            {new Date(message.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
          </span>
        </div>
        <RichTextDisplay content={message.content} />
      </div>
    </div>
  );
}

export default function ThreadPanel({ parentMessage, members, channels, onClose }: Props) {
  const { data: replies = [], isLoading: loading } = useThreadMessages(parentMessage.id);
  const sendReply = useSendThreadReply(parentMessage.id, parentMessage.channel_id);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [replies]);

  const handleSend = async (content: string) => {
    await sendReply.mutateAsync(content);
  };

  const { getUser } = useUserCache();
  const parentName = displayNameOf(getUser(parentMessage.user_id)?.display_name);

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 flex flex-col border-l border-slate-700/50 bg-slate-900/80"
      data-qa="thread-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700/50 shrink-0">
        <h3 className="text-sm font-bold text-white flex items-center gap-2">
          <MessageSquare className="w-4 h-4" />
          Thread
        </h3>
        <button
          onClick={onClose}
          aria-label="Close thread"
          className="text-slate-400 hover:text-white transition"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="px-4 py-3 border-b border-slate-700/30">
        <div className="flex items-start gap-2.5">
          <div
            className={`w-8 h-8 rounded-full ${avatarColorFor(parentMessage.user_id)} flex items-center justify-center text-sm font-bold shrink-0`}
          >
            {parentName.charAt(0).toUpperCase()}
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-baseline gap-2">
              <span className="text-sm font-semibold text-slate-200">{parentName}</span>
              <span className="text-xs text-slate-400">
                {new Date(parentMessage.created_at).toLocaleTimeString([], {
                  hour: '2-digit',
                  minute: '2-digit',
                })}
              </span>
            </div>
            <RichTextDisplay content={parentMessage.content} />
          </div>
        </div>
        <div className="mt-2 text-xs text-slate-400">
          {replies.length} {replies.length === 1 ? 'reply' : 'replies'}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-2">
        {loading ? (
          <div className="flex justify-center py-4">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : replies.length === 0 ? (
          <div className="text-center py-8 text-slate-400 text-sm">
            No replies yet. Start the conversation!
          </div>
        ) : (
          replies.map((r) => <ThreadMessage key={r.id} message={r} />)
        )}
        <div ref={endRef} />
      </div>

      <div className="border-t border-slate-700/50">
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
