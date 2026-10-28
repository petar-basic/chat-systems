import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { MessageSquare, Trash2 } from 'lucide-react';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import type { Message, WorkspaceMember, Channel } from '@/stores/workspace';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import {
  useFollowThread,
  useLiveMessage,
  useThreadMessages,
  useSendThreadReply,
  useThreadReplyActions,
  useThreadState,
} from '@/features/threads/hooks/useThreads';
import { Button } from '@/shared/components/Button/Button';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import {
  RichTextDisplay,
  MessageInput,
  MessageItem,
  asAttachments,
  useComposerAttachments,
  GalleryProvider,
} from '@/features/messaging';
import { useChannelModeration } from '@/features/channel';
import { useQueryClient } from '@tanstack/react-query';
import { useUnsent, withUnsent } from '@/stores/unsent';
import { discardUnsent } from '@/lib/unsent';
import { isClosedThread } from './closedThread';

interface Props {
  parentMessage: Message;
  replyId?: string;
  members?: WorkspaceMember[];
  channels?: Channel[];
  onClose: () => void;
}

export default function ThreadPanel({ parentMessage: opened, replyId, members, channels, onClose }: Props) {
  const parentMessage = useLiveMessage(opened);
  const {
    data: loaded = [],
    isLoading: loading,
    hasNextPage,
    fetchNextPage,
    isFetchingNextPage,
  } = useThreadMessages(parentMessage.id);
  const unsent = useUnsent(parentMessage.id);
  const replies = useMemo(() => withUnsent(loaded, unsent), [loaded, unsent]);
  const queryClient = useQueryClient();
  const [alsoToChannel, setAlsoToChannel] = useState(false);
  const currentUserId = useWorkspaceStore((s) => s.currentUserId) ?? '';
  const sendReply = useSendThreadReply(parentMessage.id, parentMessage.channel_id, currentUserId);
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { toggleReaction, edit, remove, togglePin } = useThreadReplyActions(parentMessage.id, currentUserId);
  const endRef = useRef<HTMLDivElement>(null);
  const repliesRef = useRef<HTMLDivElement>(null);
  const landedOn = useRef<string | null>(null);
  const { data: threadState, refetch: markRead } = useThreadState(parentMessage.id);
  const currentChannel = useWorkspaceStore((s) => s.currentChannel);
  const channel =
    channels?.find((ch) => ch.id === parentMessage.channel_id) ??
    (currentChannel?.id === parentMessage.channel_id ? currentChannel : undefined);
  const { canBroadcast, members: channelMembers } = useChannelModeration(
    parentMessage.channel_id,
    channel?.settings,
  );
  const follow = useFollowThread(parentMessage.id);
  const seenNewest = useRef<string | undefined | null>(null);
  const scrolledTo = useRef<string | undefined | null>(null);
  const keptFromBottom = useRef<number | null>(null);
  const newestId = replies.at(-1)?.id;
  const oldestId = replies[0]?.id;

  useEffect(() => {
    if (loading) return;
    if (seenNewest.current !== null && newestId !== seenNewest.current) void markRead();
    seenNewest.current = newestId;
  }, [loading, newestId, markRead]);

  useEffect(() => {
    if (loading) return;
    if (replyId && landedOn.current !== replyId) {
      const reply = repliesRef.current?.querySelector(`[data-message-id="${replyId}"]`);
      if (reply) {
        landedOn.current = replyId;
        scrolledTo.current = newestId;
        reply.scrollIntoView({ block: 'center' });
        return;
      }
      if (hasNextPage) {
        if (!isFetchingNextPage) void fetchNextPage();
        return;
      }
      landedOn.current = replyId;
    }
    if (scrolledTo.current === newestId) return;
    scrolledTo.current = newestId;
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [loading, replies, replyId, newestId, hasNextPage, isFetchingNextPage, fetchNextPage]);

  useLayoutEffect(() => {
    const box = repliesRef.current;
    if (!box || keptFromBottom.current === null) return;
    box.scrollTop = box.scrollHeight - keptFromBottom.current;
    keptFromBottom.current = null;
  }, [oldestId]);

  const showEarlier = () => {
    const box = repliesRef.current;
    keptFromBottom.current = box ? box.scrollHeight - box.scrollTop : null;
    void fetchNextPage();
  };

  const attachments = useComposerAttachments({
    workspaceId: currentWorkspace?.id,
    instanceUrl: currentWorkspace?.instanceUrl,
    resetKey: parentMessage.id,
  });
  const handleSend = async (content: string) => {
    sendReply.mutate({
      content,
      id: crypto.randomUUID(),
      attachments: asAttachments(attachments.take()),
      alsoToChannel,
    });
    setAlsoToChannel(false);
  };
  const replyCount = hasNextPage ? parentMessage.reply_count : loaded.filter((r) => !r.failed).length;
  const retry = (reply: Message) =>
    sendReply.mutate({
      content: reply.content,
      id: reply.id,
      attachments: reply.metadata?.attachments ?? [],
      alsoToChannel: !!reply.metadata?.also_in_channel,
    });
  const discard = (reply: Message) => discardUnsent(queryClient, reply, `thread:${parentMessage.id}`);

  const { getUser } = useUserCache();
  const parentSender = getUser(parentMessage.user_id);
  const parentName = displayNameOf(parentSender?.display_name);
  const threadMessages = useMemo(() => [parentMessage, ...replies], [parentMessage, replies]);

  return (
    <GalleryProvider messages={threadMessages}>
      <div
        className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 flex flex-col border-l border-line/50 bg-app lg:bg-app/80"
        data-qa="thread-panel"
        data-message-pane
      >
        <PanelHeader
          title="Thread"
          icon={<MessageSquare className="h-4 w-4 text-muted" />}
          actions={
            threadState && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => follow.mutate(!threadState.following)}
                disabled={follow.isPending}
                aria-pressed={threadState.following}
                data-qa="thread-follow"
              >
                {threadState.following ? 'Unfollow' : 'Follow'}
              </Button>
            )
          }
          onClose={onClose}
        />

        <div className="px-4 py-3 border-b border-line/30">
          <div className="flex items-start gap-2.5">
            <Avatar userId={parentMessage.user_id} name={parentName} avatarUrl={parentSender?.avatar_url} />
            <div className="flex-1 min-w-0">
              <div className="flex items-baseline gap-2">
                <span className="text-sm font-semibold text-fg-soft">{parentName}</span>
                <span className="text-xs text-muted">
                  {new Date(parentMessage.created_at).toLocaleTimeString([], {
                    hour: '2-digit',
                    minute: '2-digit',
                  })}
                </span>
              </div>
              {parentMessage.deleted_at ? (
                <p
                  className="text-sm text-muted italic flex items-center gap-1.5"
                  data-qa="thread-parent-deleted"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                  This message was deleted
                </p>
              ) : (
                <RichTextDisplay
                  content={parentMessage.content}
                  attachments={parentMessage.metadata?.attachments}
                />
              )}
            </div>
          </div>
          <div className="mt-2 text-xs text-muted" data-qa="thread-reply-count">
            {replyCount} {replyCount === 1 ? 'reply' : 'replies'}
          </div>
        </div>

        <div ref={repliesRef} className="flex-1 overflow-y-auto px-4 py-2">
          {loading ? (
            <div className="flex justify-center py-4">
              <Spinner className="text-accent" />
            </div>
          ) : replies.length === 0 ? (
            <div className="text-center py-8 text-muted text-sm">No replies yet. Start the conversation!</div>
          ) : (
            <>
              {hasNextPage && (
                <div className="flex justify-center py-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    loading={isFetchingNextPage}
                    onClick={showEarlier}
                    data-qa="thread-show-earlier"
                  >
                    Show earlier replies
                  </Button>
                </div>
              )}
              {replies.map((r) => {
                const sender = getUser(r.user_id);
                return (
                  <MessageItem
                    key={r.id}
                    message={r}
                    dataQa="thread-reply"
                    isHighlighted={r.id === replyId}
                    currentUserId={currentUserId}
                    senderName={displayNameOf(sender?.display_name)}
                    senderAvatarUrl={sender?.avatar_url}
                    members={members}
                    channels={channels}
                    onToggleReaction={toggleReaction}
                    onTogglePin={togglePin}
                    onEdit={edit}
                    onDelete={remove}
                    onRetry={retry}
                    onDiscard={discard}
                  />
                );
              })}
            </>
          )}
          <div ref={endRef} />
        </div>

        <div className="border-t border-line/50">
          {channel?.is_archived ? (
            <p data-qa="thread-archived" className="px-4 py-3 text-sm text-muted">
              #{channel.name} is archived, so its threads take no new replies.
            </p>
          ) : isClosedThread(parentMessage, loaded, loading) ? (
            <p data-qa="thread-closed" className="px-4 py-3 text-sm text-muted">
              This thread was deleted, so it takes no new replies.
            </p>
          ) : (
            <>
              <Checkbox
                label={channel ? `Also send to #${channel.name}` : 'Also send to the conversation'}
                checked={alsoToChannel}
                onChange={setAlsoToChannel}
                dataQa="thread-also-to-channel"
                className="px-4 pt-2 text-xs text-muted"
              />
              <MessageInput
                key={`thread:${parentMessage.id}`}
                channelName=""
                placeholder="Reply…"
                members={members}
                channels={channels}
                canBroadcast={canBroadcast}
                channelMemberCount={channelMembers.length}
                draftKey={`thread:${parentMessage.id}`}
                onSend={handleSend}
                attachments={attachments}
                workspaceId={currentWorkspace?.id}
                instanceUrl={currentWorkspace?.instanceUrl}
                scheduleTarget={{
                  channelId: parentMessage.channel_id,
                  threadParentId: parentMessage.id,
                  alsoToChannel,
                }}
                pollTarget={{
                  channelId: parentMessage.channel_id,
                  threadParentId: parentMessage.id,
                  instanceUrl: currentWorkspace?.instanceUrl,
                }}
              />
            </>
          )}
        </div>
      </div>
    </GalleryProvider>
  );
}
