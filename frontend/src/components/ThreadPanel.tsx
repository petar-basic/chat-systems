import { useEffect, useMemo, useRef } from 'react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { MessageSquare } from 'lucide-react';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import type { Message, WorkspaceMember, Channel } from '@/stores/workspace';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useThreadMessages, useSendThreadReply, useThreadReplyActions } from '@/hooks/queries/useThreads';
import RichTextDisplay from './RichTextDisplay';
import { MessageInput, MessageItem } from '@/features/messaging';
import { asAttachments, useComposerAttachments } from '@/features/messaging/useComposerAttachments';
import { GalleryProvider } from '@/features/messaging/GalleryProvider';

interface Props {
  parentMessage: Message;
  members?: WorkspaceMember[];
  channels?: Channel[];
  onClose: () => void;
}

export default function ThreadPanel({ parentMessage, members, channels, onClose }: Props) {
  const { data: replies = [], isLoading: loading } = useThreadMessages(parentMessage.id);
  const currentUserId = useWorkspaceStore((s) => s.currentUserId) ?? '';
  const sendReply = useSendThreadReply(parentMessage.id, parentMessage.channel_id, currentUserId);
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { toggleReaction, edit, remove, togglePin } = useThreadReplyActions(parentMessage.id, currentUserId);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [replies]);

  const attachments = useComposerAttachments({
    workspaceId: currentWorkspace?.id,
    instanceUrl: currentWorkspace?.instanceUrl,
    resetKey: parentMessage.id,
  });
  const handleSend = async (content: string) => {
    sendReply.mutate({ content, id: crypto.randomUUID(), attachments: asAttachments(attachments.take()) });
  };

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
              <RichTextDisplay
                content={parentMessage.content}
                attachments={parentMessage.metadata?.attachments}
              />
            </div>
          </div>
          <div className="mt-2 text-xs text-muted">
            {replies.length} {replies.length === 1 ? 'reply' : 'replies'}
          </div>
        </div>

        <div className="flex-1 overflow-y-auto px-4 py-2">
          {loading ? (
            <div className="flex justify-center py-4">
              <Spinner className="text-accent" />
            </div>
          ) : replies.length === 0 ? (
            <div className="text-center py-8 text-muted text-sm">No replies yet. Start the conversation!</div>
          ) : (
            replies.map((r) => {
              const sender = getUser(r.user_id);
              return (
                <MessageItem
                  key={r.id}
                  message={r}
                  dataQa="thread-reply"
                  currentUserId={currentUserId}
                  senderName={displayNameOf(sender?.display_name)}
                  senderAvatarUrl={sender?.avatar_url}
                  members={members}
                  channels={channels}
                  onToggleReaction={toggleReaction}
                  onTogglePin={togglePin}
                  onEdit={edit}
                  onDelete={remove}
                />
              );
            })
          )}
          <div ref={endRef} />
        </div>

        <div className="border-t border-line/50">
          <MessageInput
            key={`thread:${parentMessage.id}`}
            channelName=""
            placeholder="Reply…"
            members={members}
            channels={channels}
            draftKey={`thread:${parentMessage.id}`}
            onSend={handleSend}
            attachments={attachments}
          />
        </div>
      </div>
    </GalleryProvider>
  );
}
