import { CheckCheck, Hash, Lock, Menu, BellOff, MessageCircle } from 'lucide-react';
import type { KeyboardEvent } from 'react';
import { RichTextDisplay } from '@/features/messaging';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { conversationTitle } from '@/lib/conversationHelpers';
import type { Conversation } from '@/features/messaging';
import { useUnreads, useMarkGroupRead, type UnreadGroup } from '@/features/unreads/hooks/useUnreads';

interface Props {
  workspaceId: string;
  currentUserId: string;
  conversations: Conversation[];
  onOpenChannel: (channelId: string) => void;
  onOpenConversation: (conversationId: string) => void;
  onOpenNav?: () => void;
}

export function AllUnreadsView({
  workspaceId,
  currentUserId,
  conversations,
  onOpenChannel,
  onOpenConversation,
  onOpenNav,
}: Props) {
  const unreads = useUnreads(workspaceId);
  const markRead = useMarkGroupRead(workspaceId);
  const { getUser } = useUserCache();
  const groups = unreads.data?.pages.flatMap((page) => page.data) ?? [];
  const total = groups.reduce((sum, group) => sum + group.unread_count, 0);

  const titleOf = (group: UnreadGroup) => {
    if (group.channel_type === 'dm' || group.channel_type === 'group_dm') {
      const conversation = conversations.find((c) => c.id === group.channel_id);
      return conversation
        ? conversationTitle(conversation, currentUserId, (id) => getUser(id)?.display_name)
        : 'Direct message';
    }
    return group.channel_name ?? 'channel';
  };

  const open = (group: UnreadGroup) =>
    group.channel_type === 'dm' || group.channel_type === 'group_dm'
      ? onOpenConversation(group.channel_id)
      : onOpenChannel(group.channel_id);

  const onGroupKey = (event: KeyboardEvent<HTMLElement>, group: UnreadGroup) => {
    if (event.key !== 'Escape' || event.target !== event.currentTarget) return;
    event.preventDefault();
    event.stopPropagation();
    markRead.mutate(group);
  };

  return (
    <main className="flex-1 flex flex-col min-w-0" aria-label="All unreads" data-qa="all-unreads">
      <div className="h-14 px-4 flex items-center gap-2 border-b border-line/50 bg-surface/30 shrink-0">
        {onOpenNav && (
          <button
            onClick={onOpenNav}
            aria-label="Open navigation"
            className="lg:hidden p-1.5 -ml-1 rounded-lg text-muted hover:text-fg hover:bg-raised/50"
          >
            <Menu className="w-5 h-5" />
          </button>
        )}
        <h1 className="font-semibold text-fg">All unreads</h1>
        {total > 0 && <span className="text-sm text-muted">{total} new</span>}
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
        {unreads.isLoading && <p className="text-sm text-muted">Loading…</p>}
        {unreads.isError && <p className="text-sm text-danger">Couldn't load your unread messages.</p>}
        {unreads.isSuccess && groups.length === 0 && (
          <div
            className="flex flex-col items-center justify-center py-16 text-center"
            data-qa="all-unreads-empty"
          >
            <CheckCheck className="w-10 h-10 text-muted mb-3" />
            <p className="text-fg-soft font-medium">You're all caught up</p>
            <p className="text-sm text-muted">Nothing new in your channels or direct messages.</p>
          </div>
        )}

        {groups.map((group) => (
          <section
            key={group.channel_id}
            tabIndex={0}
            onKeyDown={(event) => onGroupKey(event, group)}
            aria-label={`${titleOf(group)}, ${group.unread_count} unread`}
            data-qa="all-unreads-group"
            className="rounded-lg border border-line/60 bg-surface/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500"
          >
            <header className="flex items-center gap-2 px-3 py-2 border-b border-line/50">
              <GroupIcon group={group} />
              <button
                type="button"
                onClick={() => open(group)}
                className="font-semibold text-sm text-fg hover:underline truncate"
              >
                {titleOf(group)}
              </button>
              {group.muted && <BellOff className="w-3.5 h-3.5 text-muted shrink-0" aria-label="Muted" />}
              <span className="text-xs text-muted shrink-0">{group.unread_count} new</span>
              <button
                type="button"
                onClick={() => markRead.mutate(group)}
                disabled={markRead.isPending && markRead.variables?.channel_id === group.channel_id}
                data-qa="all-unreads-mark-read"
                className="ml-auto text-xs px-2 py-1 rounded-md text-fg-dim hover:bg-raised hover:text-fg transition disabled:opacity-50"
              >
                Mark as read
              </button>
            </header>
            <ol className="px-3 py-2 space-y-3">
              {group.unread_count > group.messages.length && (
                <li className="text-xs text-muted">
                  {group.unread_count - group.messages.length} earlier{' '}
                  <button type="button" onClick={() => open(group)} className="text-accent hover:underline">
                    in the channel
                  </button>
                </li>
              )}
              {group.messages.map((message) => {
                const sender = getUser(message.user_id);
                const name = displayNameOf(sender?.display_name);
                return (
                  <li key={message.id} className="flex items-start gap-3">
                    <Avatar
                      userId={message.user_id}
                      name={name}
                      avatarUrl={sender?.avatar_url}
                      className="mt-0.5"
                    />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-baseline gap-2">
                        <span className="text-sm font-semibold text-fg-soft">{name}</span>
                        <span className="text-xs text-muted">
                          {new Date(message.created_at).toLocaleString([], {
                            month: 'short',
                            day: 'numeric',
                            hour: '2-digit',
                            minute: '2-digit',
                          })}
                        </span>
                      </div>
                      <RichTextDisplay
                        content={message.content}
                        attachments={message.metadata?.attachments}
                      />
                    </div>
                  </li>
                );
              })}
            </ol>
          </section>
        ))}

        {unreads.hasNextPage && (
          <button
            type="button"
            onClick={() => void unreads.fetchNextPage()}
            disabled={unreads.isFetchingNextPage}
            className="w-full py-2 text-sm text-accent hover:bg-raised/40 rounded-lg transition disabled:opacity-50"
          >
            {unreads.isFetchingNextPage ? 'Loading…' : 'Show more channels'}
          </button>
        )}
      </div>
    </main>
  );
}

function GroupIcon({ group }: { group: UnreadGroup }) {
  if (group.channel_type === 'dm' || group.channel_type === 'group_dm') {
    return <MessageCircle className="w-4 h-4 text-muted shrink-0" />;
  }
  if (group.channel_type === 'private') return <Lock className="w-4 h-4 text-muted shrink-0" />;
  return <Hash className="w-4 h-4 text-muted shrink-0" />;
}
