import { useMemo, useState } from 'react';
import { Hash, MessageSquare, Search } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import type { Channel } from '@/stores/workspace';
import type { Conversation } from '@/hooks/queries/useConversations';
import { conversationTitle } from '@/lib/conversationHelpers';
import { useUserCache } from '@/stores/users';
import { forwardedBody } from './forwardBody';

export interface ForwardSource {
  content: string;
  authorName: string;
  origin: string;
}

interface Props {
  source: ForwardSource;
  channels: Channel[];
  conversations: Conversation[];
  currentUserId: string;
  instanceUrl?: string;
  onClose: () => void;
}

export default function ForwardMessageModal({
  source,
  channels,
  conversations,
  currentUserId,
  instanceUrl,
  onClose,
}: Props) {
  const { getUser } = useUserCache();
  const [query, setQuery] = useState('');
  const [comment, setComment] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const targets = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const channelTargets = channels.map((channel) => ({
      key: `channel:${channel.id}`,
      label: `#${channel.name}`,
      isChannel: true,
      channelId: channel.id,
      conversationId: undefined as string | undefined,
    }));
    const conversationTargets = conversations.map((conversation) => ({
      key: `conversation:${conversation.id}`,
      label: conversationTitle(conversation, currentUserId, (id) => getUser(id)?.display_name),
      isChannel: false,
      channelId: undefined as string | undefined,
      conversationId: conversation.id,
    }));
    return [...channelTargets, ...conversationTargets].filter((target) =>
      needle ? target.label.toLowerCase().includes(needle) : true,
    );
  }, [channels, conversations, currentUserId, getUser, query]);

  const forwardTo = async (target: { channelId?: string; conversationId?: string }) => {
    setSending(true);
    setError(null);
    try {
      const content = forwardedBody(source, comment);
      const api = getApiForInstance(instanceUrl);
      if (target.channelId) {
        await api.post(`/channels/${target.channelId}/messages`, {
          content,
          client_message_id: crypto.randomUUID(),
        });
      } else {
        await api.post(`/conversations/${target.conversationId}/messages`, {
          content,
          client_message_id: crypto.randomUUID(),
        });
      }
      onClose();
    } catch (err) {
      setError((err as { message?: string })?.message || 'Failed to forward that message');
    } finally {
      setSending(false);
    }
  };

  return (
    <Modal title="Forward message" onClose={onClose} dataQa="forward-modal">
      <div className="px-4 py-3 border-b border-slate-700/50">
        <div className="text-xs text-slate-400 mb-1">
          From {source.authorName} in {source.origin}
        </div>
        <div className="text-sm text-slate-300 line-clamp-3 whitespace-pre-wrap" data-qa="forward-preview">
          {source.content}
        </div>
      </div>

      <div className="px-4 py-3 space-y-2">
        <input
          value={comment}
          onChange={(e) => setComment(e.target.value)}
          placeholder="Add a comment (optional)"
          aria-label="Comment"
          data-qa="forward-comment"
          className="w-full px-3 py-2 bg-slate-700/50 border border-slate-600 rounded-lg text-sm text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
        />
        <div className="relative">
          <Search className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search channels and people"
            aria-label="Search for somewhere to forward to"
            data-qa="forward-search"
            className="w-full pl-9 pr-3 py-2 bg-slate-700/50 border border-slate-600 rounded-lg text-sm text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
        </div>
        {error && (
          <p className="text-xs text-red-400" data-qa="forward-error">
            {error}
          </p>
        )}
      </div>

      <div className="max-h-72 overflow-y-auto px-2 pb-3">
        {targets.length === 0 ? (
          <p className="px-3 py-6 text-center text-sm text-slate-400">Nothing matches that.</p>
        ) : (
          targets.map((target) => (
            <button
              key={target.key}
              disabled={sending}
              onClick={() =>
                forwardTo({ channelId: target.channelId, conversationId: target.conversationId })
              }
              data-qa="forward-target"
              className="w-full flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-slate-700/30 transition cursor-pointer text-left disabled:opacity-50"
            >
              {target.isChannel ? (
                <Hash className="w-4 h-4 text-slate-400 shrink-0" />
              ) : (
                <MessageSquare className="w-4 h-4 text-slate-400 shrink-0" />
              )}
              <span className="text-sm text-slate-200 truncate">{target.label}</span>
            </button>
          ))
        )}
      </div>
    </Modal>
  );
}
