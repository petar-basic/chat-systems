import { useTranslation } from 'react-i18next';
import { Hash, MessageSquare, Paperclip } from 'lucide-react';
import type { Channel } from '@/stores/workspace';
import { useWorkspaceStore } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging';
import { useUserCache } from '@/stores/users';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { formatDateTime } from '@/lib/datetime';
import { toPlainText } from '@/lib/plainText';
import { targetLabel } from '@/lib/targetLabel';
import { useSentMessages } from './hooks/useSentMessages';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  conversations: Conversation[];
  onOpenMessage: (messageId: string, thread?: boolean) => void;
}

export function SentList({ workspaceId, instanceUrl, channels, conversations, onOpenMessage }: Props) {
  const { t } = useTranslation('messages');
  const { data, isLoading, isError, hasNextPage, fetchNextPage, isFetchingNextPage } = useSentMessages(
    workspaceId,
    instanceUrl,
  );
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);
  const { getUser } = useUserCache();
  const sent = data?.pages.flatMap((page) => page.data) ?? [];

  if (isLoading) {
    return (
      <div className="flex justify-center py-8">
        <Spinner className="text-accent" />
      </div>
    );
  }
  if (isError) {
    return (
      <div className="text-center py-10 px-6 text-sm text-danger" data-qa="sent-error">
        {t('sent.failed')}
      </div>
    );
  }
  if (sent.length === 0) {
    return (
      <div className="text-center py-10 px-6 text-sm text-muted" data-qa="sent-empty">
        {t('sent.empty')}
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto" data-qa="sent-list">
      {sent.map((message) => {
        const inConversation = conversations.some((c) => c.id === message.channel_id);
        const label = targetLabel(
          message.channel_id,
          channels,
          conversations,
          currentUserId ?? undefined,
          (id) => getUser(id)?.display_name,
          t('scheduled.leftChannel'),
        );
        const files = message.metadata?.attachments?.length ?? 0;
        return (
          <button
            key={message.id}
            type="button"
            onClick={() => onOpenMessage(message.id, !!message.thread_parent_id)}
            className="w-full text-left px-4 py-3 border-b border-line/40 hover:bg-raised/30 transition"
            data-qa="sent-row"
            data-message-id={message.id}
          >
            <span className="text-xs text-muted flex items-center gap-1.5" data-qa="sent-target">
              {inConversation ? (
                <MessageSquare className="w-3 h-3 shrink-0" />
              ) : (
                <Hash className="w-3 h-3 shrink-0" />
              )}
              <span className="truncate">
                {message.thread_parent_id ? t('scheduled.thread', { target: label }) : label}
              </span>
              <span className="ml-auto shrink-0">{formatDateTime(message.created_at)}</span>
            </span>
            {message.content && (
              <span className="block text-sm text-fg-soft mt-0.5 line-clamp-2" data-qa="sent-content">
                {toPlainText(message.content)}
              </span>
            )}
            {files > 0 && (
              <span className="text-xs text-muted mt-0.5 flex items-center gap-1">
                <Paperclip className="w-3 h-3 shrink-0" />
                {t('scheduled.files', { count: files })}
              </span>
            )}
          </button>
        );
      })}
      {hasNextPage && (
        <button
          type="button"
          onClick={() => void fetchNextPage()}
          disabled={isFetchingNextPage}
          className="w-full py-3 text-xs text-accent hover:underline disabled:opacity-50"
          data-qa="sent-more"
        >
          {isFetchingNextPage ? t('sent.loading') : t('sent.more')}
        </button>
      )}
    </div>
  );
}
