import { X, Pin } from 'lucide-react';
import type { Message } from '../stores/workspace';
import { useUserCache } from '../stores/users';
import { useChannelPins } from '../hooks/queries/useChannels';
import RichTextDisplay from './RichTextDisplay';

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
      className="w-full text-left px-4 py-3 border-b border-slate-700/50 hover:bg-slate-700/20 transition cursor-pointer"
    >
      <div className="flex items-baseline gap-2 mb-1">
        <span className="text-sm font-semibold text-slate-200">{displayName}</span>
        <span className="text-xs text-slate-400">
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
    <div className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-slate-800 border-l border-slate-700/50 flex flex-col">
      <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700/50 shrink-0">
        <div className="flex items-center gap-2">
          <Pin className="w-4 h-4 text-amber-400" />
          <span className="font-semibold">Pinned Messages</span>
        </div>
        <button onClick={onClose} className="text-slate-400 hover:text-white transition cursor-pointer">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : messages.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-slate-400 px-4 text-center">
            <Pin className="w-8 h-8 mb-2 text-slate-600" />
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
