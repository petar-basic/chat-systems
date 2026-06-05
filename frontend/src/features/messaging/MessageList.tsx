import { Fragment, useCallback, useEffect, useMemo, useRef } from 'react';
import { MessageSquare } from 'lucide-react';
import MessageItem from './MessageItem';
import type { Message, WorkspaceMember, Channel } from '@/stores/workspace';
import { useMessages } from '@/hooks/queries/useMessages';
import { useUserCache } from '@/stores/users';
import { avatarColorFor, displayNameOf } from '@/lib/userHelpers';
import { useMessageActions } from './hooks/useMessageActions';
import { isGrouped, isNewDay, formatDaySeparator } from './messageGrouping';
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

export default function MessageList({
  channelId,
  members,
  channels,
  onThreadOpen,
  highlightMessageId,
  onTargetMessageFound,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const { data, isLoading, isError, refetch, isFetchingNextPage, hasNextPage, fetchNextPage } =
    useMessages(channelId);
  const scrolledToRef = useRef<string | undefined>(undefined);

  const { getUser } = useUserCache();
  const actions = useMessageActions(channelId);

  useEffect(() => {
    scrolledToRef.current = undefined;
  }, [highlightMessageId, channelId]);

  const messages = useMemo(() => data?.pages.flatMap((page) => page.data) ?? [], [data]);

  useEffect(() => {
    if (!highlightMessageId || !data) return;
    const found = messages.find((m) => m.id === highlightMessageId);
    if (found) {
      onTargetMessageFound?.(found);
      if (scrolledToRef.current !== highlightMessageId) {
        scrolledToRef.current = highlightMessageId;
        requestAnimationFrame(() => {
          const el = containerRef.current?.querySelector(`[data-message-id="${highlightMessageId}"]`);
          el?.scrollIntoView({ behavior: 'smooth', block: 'center' });
        });
      }
    } else if (hasNextPage && !isFetchingNextPage) {
      fetchNextPage();
    }
  }, [
    messages,
    highlightMessageId,
    data,
    hasNextPage,
    isFetchingNextPage,
    fetchNextPage,
    onTargetMessageFound,
  ]);

  const handleScroll = useCallback(() => {
    if (!containerRef.current || isFetchingNextPage || !hasNextPage) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    if (scrollHeight + scrollTop - clientHeight < 100) fetchNextPage();
  }, [isFetchingNextPage, hasNextPage, fetchNextPage]);

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
      <div
        ref={containerRef}
        data-qa="message-list"
        role="log"
        aria-live="polite"
        className="flex-1 overflow-y-auto px-4 py-4 flex flex-col-reverse"
        onScroll={handleScroll}
      >
        <div className="space-y-0.5">
          {messages.map((msg, i) => {
            const prev = messages[i - 1];
            const sender = getUser(msg.user_id);
            const newDay = isNewDay(prev, msg);
            const grouped = !newDay && isGrouped(prev, msg);
            return (
              <Fragment key={msg.id}>
                {newDay && (
                  <div className="flex items-center gap-3 px-2 py-2" data-qa="day-separator">
                    <div className="flex-1 h-px bg-slate-700/60" />
                    <span className="text-xs font-medium text-slate-400">
                      {formatDaySeparator(msg.created_at)}
                    </span>
                    <div className="flex-1 h-px bg-slate-700/60" />
                  </div>
                )}
                <MessageItem
                  message={msg}
                  grouped={grouped}
                  members={members}
                  channels={channels}
                  currentUserId={actions.currentUserId}
                  senderName={displayNameOf(sender?.display_name)}
                  avatarColor={avatarColorFor(msg.user_id)}
                  isHighlighted={msg.id === highlightMessageId}
                  onThreadOpen={onThreadOpen}
                  onToggleReaction={actions.toggleReaction}
                  onTogglePin={actions.togglePin}
                  onEdit={actions.editMessage}
                  onDelete={actions.deleteMessage}
                  onRetry={actions.retryMessage}
                  onCopyLink={actions.copyLink}
                />
              </Fragment>
            );
          })}
        </div>
        {isFetchingNextPage && (
          <div className="flex justify-center py-2">
            <div className="w-4 h-4 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        )}
      </div>
    </QueryState>
  );
}
