import { useCallback, useEffect, useMemo, useState } from 'react';
import { MessageSquare } from 'lucide-react';
import MessageItem from './MessageItem';
import VirtualMessageList from './VirtualMessageList';
import { buildMessageRows, type MessageRow } from './messageRows';
import type { Message, WorkspaceMember, Channel } from '@/stores/workspace';
import { useMessages, messagesOldestFirst } from '@/hooks/queries/useMessages';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { useMessageActions } from './hooks/useMessageActions';
import { formatDaySeparator } from './messageGrouping';
import { QueryState } from '@/shared/components/QueryState/QueryState';
import { EmptyLabels } from '@/shared/constants';

interface Props {
  channelId: string;
  members?: WorkspaceMember[];
  channels?: Channel[];
  onThreadOpen: (msg: Message) => void;
  highlightMessageId?: string;
  onTargetMessageFound?: (msg: Message) => void;
}

export function DaySeparator({ at }: { at: string }) {
  return (
    <div className="flex items-center gap-3 px-2 py-2" data-qa="day-separator">
      <div className="flex-1 h-px bg-slate-700/60" />
      <span className="text-xs font-medium text-slate-400">{formatDaySeparator(at)}</span>
      <div className="flex-1 h-px bg-slate-700/60" />
    </div>
  );
}

export default function MessageList({
  channelId,
  members,
  channels,
  onThreadOpen,
  highlightMessageId,
  onTargetMessageFound,
}: Props) {
  const { data, isLoading, isError, refetch, isFetchingNextPage, hasNextPage, fetchNextPage } =
    useMessages(channelId);
  const { getUser } = useUserCache();
  const actions = useMessageActions(channelId);

  // Reset during render rather than in an effect, the way `useRightPanel` does:
  // a new permalink target should be pending before the first paint, not after.
  const targetKey = `${channelId}:${highlightMessageId ?? ''}`;
  const [scrollTarget, setScrollTarget] = useState<string | undefined>(highlightMessageId);
  const [lastTargetKey, setLastTargetKey] = useState(targetKey);
  if (targetKey !== lastTargetKey) {
    setLastTargetKey(targetKey);
    setScrollTarget(highlightMessageId);
  }

  const messages = useMemo(() => messagesOldestFirst(data), [data]);
  const rows = useMemo(() => buildMessageRows(messages), [messages]);

  // A permalink can point at a message older than anything loaded, so keep
  // pulling pages until it turns up; `scrollToKey` does the rest once it does.
  useEffect(() => {
    if (!highlightMessageId || !data) return;
    const found = messages.find((m) => m.id === highlightMessageId);
    if (found) onTargetMessageFound?.(found);
    else if (hasNextPage && !isFetchingNextPage) fetchNextPage();
  }, [
    messages,
    highlightMessageId,
    data,
    hasNextPage,
    isFetchingNextPage,
    fetchNextPage,
    onTargetMessageFound,
  ]);

  const renderRow = useCallback(
    (row: MessageRow<Message>) => {
      if (row.kind === 'day') return <DaySeparator at={row.at} />;
      const sender = getUser(row.message.user_id);
      return (
        <MessageItem
          message={row.message}
          grouped={row.grouped}
          members={members}
          channels={channels}
          currentUserId={actions.currentUserId}
          senderName={row.message.metadata?.bot?.name ?? displayNameOf(sender?.display_name)}
          senderAvatarUrl={row.message.metadata?.bot?.icon_url ?? sender?.avatar_url}
          isHighlighted={row.message.id === highlightMessageId}
          onThreadOpen={onThreadOpen}
          onToggleReaction={actions.toggleReaction}
          onTogglePin={actions.togglePin}
          onEdit={actions.editMessage}
          onDelete={actions.deleteMessage}
          onRetry={actions.retryMessage}
          onCopyLink={actions.copyLink}
        />
      );
    },
    [actions, channels, getUser, highlightMessageId, members, onThreadOpen],
  );

  return (
    <QueryState
      isLoading={isLoading}
      isError={isError}
      isEmpty={messages.length === 0}
      onRetry={() => void refetch()}
      empty={
        <>
          <MessageSquare className="w-12 h-12 mb-3 text-slate-600" />
          <p className="text-lg font-medium">{EmptyLabels.NoMessages}</p>
          <p className="text-sm">{EmptyLabels.NoMessagesHint}</p>
        </>
      }
    >
      <VirtualMessageList
        rows={rows}
        renderRow={renderRow}
        hasOlder={!!hasNextPage}
        isLoadingOlder={isFetchingNextPage}
        onLoadOlder={fetchNextPage}
        scrollToKey={scrollTarget}
        onScrollToKeyHandled={() => setScrollTarget(undefined)}
        qa="message-list"
        ariaLabel="Messages"
      />
    </QueryState>
  );
}
