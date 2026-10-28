import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Hash, Lock, Menu, MessageCircle, MessagesSquare } from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { RichTextDisplay, MessageInput, asAttachments, useComposerAttachments } from '@/features/messaging';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useUserCache } from '@/stores/users';
import type { Channel, Message, WorkspaceMember } from '@/stores/workspace';
import { displayNameOf } from '@/lib/userHelpers';
import { conversationTitle } from '@/lib/conversationHelpers';
import type { Conversation } from '@/features/messaging';
import {
  useFollowedThreads,
  useMarkThreadRead,
  useSendThreadReply,
  type ThreadSummary,
} from '@/features/threads/hooks/useThreads';
import { QUERY_KEYS } from '@/shared/constants';
import { useUnsent } from '@/stores/unsent';
import { discardUnsent } from '@/lib/unsent';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  currentUserId: string;
  channels: Channel[];
  conversations: Conversation[];
  members: WorkspaceMember[];
  onOpenThread: (root: Message) => void;
  onOpenChannel: (channelId: string) => void;
  onOpenConversation: (conversationId: string) => void;
  onOpenNav?: () => void;
}

interface Seen {
  readAt: string;
  fresh: number;
}

const timeOf = (iso: string) =>
  new Date(iso).toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });

export function ThreadsView({
  workspaceId,
  instanceUrl,
  currentUserId,
  channels,
  conversations,
  members,
  onOpenThread,
  onOpenChannel,
  onOpenConversation,
  onOpenNav,
}: Props) {
  const threads = useFollowedThreads(workspaceId);
  const markRead = useMarkThreadRead();
  const { getUser } = useUserCache();
  const list = threads.data?.pages.flatMap((page) => page.data) ?? [];
  const [seen, setSeen] = useState<Record<string, Seen>>({});
  const marked = useRef(new Set<string>());

  const arrived = list.filter((t) => t.unread_count > 0 && seen[t.root.id]?.readAt !== t.last_read_at);
  if (arrived.length > 0) {
    setSeen((prev) => {
      const next = { ...prev };
      for (const t of arrived) {
        const before = prev[t.root.id];
        next[t.root.id] = {
          readAt: t.last_read_at,
          fresh: (before?.fresh ?? 0) + t.unread_count,
        };
      }
      return next;
    });
  }

  const { mutate } = markRead;
  useEffect(() => {
    for (const [rootId, entry] of Object.entries(seen)) {
      const key = `${rootId}:${entry.readAt}`;
      if (marked.current.has(key)) continue;
      marked.current.add(key);
      mutate(rootId);
    }
  }, [seen, mutate]);

  const placeOf = (root: ThreadSummary['root']) => {
    const channel = channels.find((c) => c.id === root.channel_id);
    if (channel) {
      return {
        title: channel.name ?? 'channel',
        icon:
          channel.channel_type === 'private' ? (
            <Lock className="w-4 h-4 text-muted shrink-0" />
          ) : (
            <Hash className="w-4 h-4 text-muted shrink-0" />
          ),
        open: () => onOpenChannel(channel.id),
      };
    }
    const conversation = conversations.find((c) => c.id === root.channel_id);
    return {
      title: conversation
        ? conversationTitle(conversation, currentUserId, (id) => getUser(id)?.display_name)
        : 'Direct message',
      icon: <MessageCircle className="w-4 h-4 text-muted shrink-0" />,
      open: () => onOpenConversation(root.channel_id),
    };
  };

  return (
    <main className="flex-1 flex flex-col min-w-0" aria-label="Threads" data-qa="threads">
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
        <h1 className="font-semibold text-fg">Threads</h1>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
        {threads.isLoading && <p className="text-sm text-muted">Loading…</p>}
        {threads.isError && <p className="text-sm text-danger">Couldn't load your threads.</p>}
        {threads.isSuccess && list.length === 0 && (
          <div
            className="flex flex-col items-center justify-center py-16 text-center"
            data-qa="threads-empty"
          >
            <MessagesSquare className="w-10 h-10 text-muted mb-3" />
            <p className="text-fg-soft font-medium">No threads yet</p>
            <p className="text-sm text-muted">
              Threads you start, reply to or are mentioned in show up here.
            </p>
          </div>
        )}

        {list.map((thread) => (
          <ThreadCard
            key={thread.root.id}
            thread={thread}
            seen={seen[thread.root.id]}
            place={placeOf(thread.root)}
            workspaceId={workspaceId}
            instanceUrl={instanceUrl}
            currentUserId={currentUserId}
            channels={channels}
            members={members}
            onOpenThread={onOpenThread}
          />
        ))}

        {threads.hasNextPage && (
          <button
            type="button"
            onClick={() => void threads.fetchNextPage()}
            disabled={threads.isFetchingNextPage}
            className="w-full py-2 text-sm text-accent hover:bg-raised/40 rounded-lg transition disabled:opacity-50"
          >
            {threads.isFetchingNextPage ? 'Loading…' : 'Show more threads'}
          </button>
        )}
      </div>
    </main>
  );
}

interface CardProps {
  thread: ThreadSummary;
  seen?: Seen;
  place: { title: string; icon: ReactNode; open: () => void };
  workspaceId: string;
  instanceUrl?: string;
  currentUserId: string;
  channels: Channel[];
  members: WorkspaceMember[];
  onOpenThread: (root: Message) => void;
}

function ThreadCard({
  thread,
  seen,
  place,
  workspaceId,
  instanceUrl,
  currentUserId,
  channels,
  members,
  onOpenThread,
}: CardProps) {
  const { root, latest_replies: replies } = thread;
  const queryClient = useQueryClient();
  const sendReply = useSendThreadReply(root.id, root.channel_id, currentUserId);
  const attachments = useComposerAttachments({ workspaceId, instanceUrl, resetKey: root.id });
  const earlier = root.reply_count - replies.length;
  const fresh = seen?.fresh ?? 0;
  const others = replies.filter((reply) => reply.user_id !== currentUserId);
  const newIds = new Set(fresh > 0 ? others.slice(-fresh).map((reply) => reply.id) : []);

  const unsent = useUnsent(root.id);
  const refreshThreads = {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: QUERY_KEYS.followedThreads(workspaceId) }),
  };

  const handleSend = async (content: string) => {
    sendReply.mutate(
      { content, id: crypto.randomUUID(), attachments: asAttachments(attachments.take()) },
      refreshThreads,
    );
  };
  const retry = (reply: Message) =>
    sendReply.mutate(
      {
        content: reply.content,
        id: reply.id,
        attachments: reply.metadata?.attachments ?? [],
        alsoToChannel: !!reply.metadata?.also_in_channel,
      },
      refreshThreads,
    );
  const discard = (reply: Message) => discardUnsent(queryClient, reply, `thread:${root.id}`);

  return (
    <section
      aria-label={`Thread in ${place.title}`}
      data-qa="thread-card"
      className="rounded-lg border border-line/60 bg-surface/40"
    >
      <header className="flex items-center gap-2 px-3 py-2 border-b border-line/50">
        {place.icon}
        <button
          type="button"
          onClick={place.open}
          className="font-semibold text-sm text-fg hover:underline truncate"
        >
          {place.title}
        </button>
        {fresh > 0 && (
          <span className="ml-auto text-xs px-1.5 rounded-full bg-accent/20 text-accent" data-qa="thread-new">
            {fresh} new {fresh === 1 ? 'reply' : 'replies'}
          </span>
        )}
      </header>
      <ol className="px-3 py-2 space-y-3">
        <ThreadLine message={root} />
        {earlier > 0 && (
          <li>
            <button
              type="button"
              onClick={() => onOpenThread(root as Message)}
              data-qa="thread-open"
              className="text-xs text-accent hover:underline"
            >
              View {earlier} more {earlier === 1 ? 'reply' : 'replies'}
            </button>
          </li>
        )}
        {replies.map((reply) => (
          <ThreadLine key={reply.id} message={reply} highlighted={newIds.has(reply.id)} />
        ))}
        {unsent.map((reply) => (
          <ThreadLine
            key={reply.id}
            message={{ ...reply, metadata: reply.metadata ?? {} }}
            failure={{ onRetry: () => retry(reply), onDiscard: () => discard(reply) }}
          />
        ))}
      </ol>
      <div className="border-t border-line/50" data-qa="thread-card-reply">
        <MessageInput
          channelName=""
          placeholder="Reply…"
          members={members}
          channels={channels}
          draftKey={`thread:${root.id}`}
          onSend={handleSend}
          attachments={attachments}
          pollTarget={{ channelId: root.channel_id, threadParentId: root.id, instanceUrl }}
        />
      </div>
    </section>
  );
}

function ThreadLine({
  message,
  highlighted = false,
  failure,
}: {
  message: ThreadSummary['root'];
  highlighted?: boolean;
  failure?: { onRetry: () => void; onDiscard: () => void };
}) {
  const { getUser } = useUserCache();
  const sender = getUser(message.user_id);
  const name = displayNameOf(sender?.display_name);
  return (
    <li
      className={`flex items-start gap-3 rounded-md ${highlighted ? 'bg-accent/10 -mx-1 px-1' : ''}`}
      data-qa={highlighted ? 'thread-reply-new' : undefined}
    >
      <Avatar userId={message.user_id} name={name} avatarUrl={sender?.avatar_url} className="mt-0.5" />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-semibold text-fg-soft">{name}</span>
          <span className="text-xs text-muted">{timeOf(message.created_at)}</span>
        </div>
        {message.deleted_at ? (
          <p className="text-sm text-muted italic" data-qa="thread-root-deleted">
            This message was deleted
          </p>
        ) : (
          <RichTextDisplay content={message.content} attachments={message.metadata?.attachments} />
        )}
        {failure && (
          <div className="mt-1 flex items-center gap-2 text-xs text-danger" data-qa="message-failed">
            <span>Failed to send.</span>
            <button
              type="button"
              onClick={failure.onRetry}
              data-qa="message-retry"
              className="font-semibold text-danger hover:text-danger underline"
            >
              Retry
            </button>
            <button
              type="button"
              onClick={failure.onDiscard}
              data-qa="message-discard"
              className="font-semibold text-muted hover:text-fg underline"
            >
              Discard
            </button>
          </div>
        )}
      </div>
    </li>
  );
}
