import { X, Pin } from 'lucide-react';
import type { Message } from '../stores/workspace';
import { useUserCache } from '../stores/users';
import { useChannelPins } from '../hooks/queries/useChannels';
import RichTextDisplay from './RichTextDisplay';
import { Avatar } from '@/shared/components/Avatar/Avatar';

interface Props {
  channelId: string;
  onClose: () => void;
  onNavigate?: (messageId: string) => void;
}

function PinnedMessageRow({ message, onNavigate }: { message: Message; onNavigate?: (id: string) => void }) {
  const { getUser } = useUserCache();
  const sender = getUser(message.user_id);
  const displayName = sender?.display_name || message.user_id.slice(0, 8);

  return (
    <button
      onClick={() => onNavigate?.(message.id)}
      className="w-full text-left px-4 py-3 border-b border-line/50 hover:bg-raised/20 transition cursor-pointer"
    >
      <div className="flex items-center gap-2 mb-1">
        <Avatar userId={message.user_id} name={displayName} avatarUrl={sender?.avatar_url} size="xs" />
        <span className="text-sm font-semibold text-fg-soft">{displayName}</span>
        <span className="text-xs text-muted">
          {new Date(message.created_at).toLocaleDateString([], { month: 'short', day: 'numeric' })}
        </span>
      </div>
      <div className="line-clamp-3">
        <RichTextDisplay content={message.content} />
      </div>
    </button>
  );
}

export default function PinnedMessagesPanel({ channelId, onClose, onNavigate }: Props) {
  const { data: messages = [], isLoading: loading } = useChannelPins(channelId);

  return (
    <div className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col">
      <div className="h-14 px-4 flex items-center justify-between border-b border-line/50 shrink-0">
        <div className="flex items-center gap-2">
          <Pin className="w-4 h-4 text-warning" />
          <span className="font-semibold">Pinned Messages</span>
        </div>
        <button onClick={onClose} className="text-muted hover:text-fg transition cursor-pointer">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : messages.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-muted px-4 text-center">
            <Pin className="w-8 h-8 mb-2 text-faint" />
            <p className="text-sm">No pinned messages yet</p>
            <p className="text-xs mt-1">Pin important messages to find them later</p>
          </div>
        ) : (
          messages.map((msg) => <PinnedMessageRow key={msg.id} message={msg} onNavigate={onNavigate} />)
        )}
      </div>
    </div>
  );
}
