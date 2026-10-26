import { useMemo, useState } from 'react';
import { TextInput } from '@/shared/components/Form/TextInput';
import { toUserMessage } from '@/lib/errors';
import { Hash, MessageSquare, Search } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import type { Channel } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging/hooks/useConversations';
import { conversationTitle } from '@/lib/conversationHelpers';
import { useUserCache } from '@/stores/users';
import { forwardedBody } from './forwardBody';

export interface ForwardSource {
  messageId: string;
  content: string;
  authorName: string;
  origin: string;
  hasAttachments: boolean;
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
    }));
    const conversationTargets = conversations.map((conversation) => ({
      key: `conversation:${conversation.id}`,
      label: conversationTitle(conversation, currentUserId, (id) => getUser(id)?.display_name),
      isChannel: false,
      channelId: conversation.id,
    }));
    return [...channelTargets, ...conversationTargets].filter((target) =>
      needle ? target.label.toLowerCase().includes(needle) : true,
    );
  }, [channels, conversations, currentUserId, getUser, query]);

  const forwardTo = async (channelId: string) => {
    setSending(true);
    setError(null);
    try {
      const content = forwardedBody(source, comment);
      const body = {
        content,
        client_message_id: crypto.randomUUID(),
        copy_attachments_from: source.hasAttachments ? source.messageId : null,
      };
      await getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/channels/{ch_id}/messages', { params: { path: { ch_id: channelId } }, body }),
      );
      onClose();
    } catch (err) {
      setError(toUserMessage(err, 'Failed to forward that message'));
    } finally {
      setSending(false);
    }
  };

  return (
    <Modal title="Forward message" onClose={onClose} dataQa="forward-modal">
      <div className="rounded-lg border border-line/60 bg-raised/30 px-3 py-2">
        <div className="text-xs text-muted mb-1">
          From {source.authorName} in {source.origin}
        </div>
        <div className="text-sm text-fg-dim line-clamp-3 whitespace-pre-wrap" data-qa="forward-preview">
          {source.content}
        </div>
        {source.hasAttachments && <div className="mt-1 text-xs text-muted">📎 Files are forwarded too</div>}
      </div>

      <div className="mt-3 space-y-2">
        <TextInput
          label="Comment"
          labelHidden
          value={comment}
          onChange={(e) => setComment(e.target.value)}
          placeholder="Add a comment (optional)"
          dataQa="forward-comment"
        />
        <TextInput
          label="Search for somewhere to forward to"
          labelHidden
          leading={<Search className="h-4 w-4" />}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search channels and people"
          dataQa="forward-search"
        />
        {error && (
          <p className="text-xs text-danger" data-qa="forward-error">
            {error}
          </p>
        )}
      </div>

      <div className="-mx-3 mt-2">
        {targets.length === 0 ? (
          <p className="px-3 py-6 text-center text-sm text-muted">Nothing matches that.</p>
        ) : (
          targets.map((target) => (
            <button
              key={target.key}
              disabled={sending}
              onClick={() => forwardTo(target.channelId)}
              data-qa="forward-target"
              className="w-full flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-raised/30 transition cursor-pointer text-left disabled:opacity-50"
            >
              {target.isChannel ? (
                <Hash className="w-4 h-4 text-muted shrink-0" />
              ) : (
                <MessageSquare className="w-4 h-4 text-muted shrink-0" />
              )}
              <span className="text-sm text-fg-soft truncate">{target.label}</span>
            </button>
          ))
        )}
      </div>
    </Modal>
  );
}
