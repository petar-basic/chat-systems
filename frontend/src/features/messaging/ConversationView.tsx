import { ArrowLeft, Menu } from 'lucide-react';
import { useUserCache } from '@/stores/users';
import { usePresenceStore } from '@/stores/presence';
import type { Channel, Message, WorkspaceMember } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging/hooks/useConversations';
import { useSendMessage } from '@/features/messaging/hooks/useMessages';
import { conversationTitle } from '@/lib/conversationHelpers';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { ConnectionBanner } from '@/shared/components/ConnectionBanner/ConnectionBanner';
import { HuddleStartButton } from '@/features/huddle';
import { NotificationLevelButton } from '@/features/notifications';
import { useTypingSignal, typingTargetOf } from '@/shared/hooks/useTypingSignal';
import { FileDropZone } from '@/shared/components/FileDropZone';
import PresenceDot from '@/shared/components/PresenceDot';
import TypingIndicator from '@/features/messaging/TypingIndicator';
import MessageList from './MessageList';
import MessageInput from './MessageInput';
import { asAttachments, useComposerAttachments } from './useComposerAttachments';

interface Props {
  workspaceId: string;
  instanceUrl: string;
  conversation: Conversation;
  currentUserId: string;
  members: WorkspaceMember[];
  channels: Channel[];
  highlightMessageId?: string;
  onTargetMessageFound?: (msg: Message) => void;
  onClose: () => void;
  onOpenNav?: () => void;
  onOpenThread: (message: Message) => void;
  onSave: (message: Message) => void;
  onForward: (message: Message) => void;
}

/// A direct message is a channel nobody can browse, so the feed, the composer
/// and the thread panel are the channel ones; only the header knows the
/// difference.
export default function ConversationView({
  workspaceId,
  instanceUrl,
  conversation,
  currentUserId,
  members,
  channels,
  highlightMessageId,
  onTargetMessageFound,
  onClose,
  onOpenNav,
  onOpenThread,
  onSave,
  onForward,
}: Props) {
  const { getUser } = useUserCache();
  const title = conversationTitle(conversation, currentUserId, (id) => getUser(id)?.display_name);
  const participantIds = conversation.participant_ids;
  const partnerId = participantIds.find((id) => id !== currentUserId) ?? currentUserId;
  const partner = getUser(partnerId);
  const status = usePresenceStore((s) => s.getStatus(partnerId));
  const isDirect = conversation.kind === 'direct';

  const sendMutation = useSendMessage(conversation.id, currentUserId);
  const { signalTyping, stopTyping } = useTypingSignal(typingTargetOf(conversation.id), instanceUrl);
  const attachments = useComposerAttachments({ workspaceId, instanceUrl, resetKey: conversation.id });
  const handleSend = async (content: string) => {
    stopTyping();
    sendMutation.mutate({ content, id: crypto.randomUUID(), attachments: asAttachments(attachments.take()) });
  };

  return (
    <FileDropZone className="flex-1 flex min-w-0" onFiles={attachments.add}>
      <div role="main" aria-label="Direct message" className="flex-1 flex flex-col min-w-0" data-message-pane>
        <ConnectionBanner instanceUrl={instanceUrl} />
        <div className="h-12 px-4 flex items-center gap-3 border-b border-line/50 shrink-0">
          {onOpenNav && (
            <button
              onClick={onOpenNav}
              aria-label="Open navigation"
              className="lg:hidden p-1.5 -ml-1 rounded-lg text-muted hover:text-fg hover:bg-raised/50"
            >
              <Menu className="w-5 h-5" />
            </button>
          )}
          <button
            onClick={onClose}
            aria-label="Back to channels"
            className="text-muted hover:text-fg transition cursor-pointer mr-1"
          >
            <ArrowLeft className="w-4 h-4" />
          </button>
          {isDirect ? (
            <div className="relative shrink-0">
              <Avatar userId={partnerId} name={title} avatarUrl={partner?.avatar_url} size="sm" />
              <PresenceDot userId={partnerId} className="absolute -bottom-0.5 -right-0.5 ring-2 ring-app" />
            </div>
          ) : (
            <div className="flex -space-x-2 shrink-0" data-qa="conversation-participants">
              {participantIds.slice(0, 3).map((id) => (
                <Avatar
                  key={id}
                  userId={id}
                  name={displayNameOf(getUser(id)?.display_name)}
                  avatarUrl={getUser(id)?.avatar_url}
                  size="xs"
                  className="ring-2 ring-app"
                />
              ))}
            </div>
          )}
          <span className="font-semibold text-fg truncate" data-qa="conversation-title">
            {title}
          </span>
          {!isDirect && <span className="text-xs text-muted shrink-0">{participantIds.length} people</span>}
          {isDirect && status === 'online' && <span className="text-xs text-muted">Active now</span>}
          <div className="ml-auto flex items-center gap-1">
            <NotificationLevelButton channelId={conversation.id} direct />
            {isDirect && (
              <HuddleStartButton
                workspaceId={workspaceId}
                instanceUrl={instanceUrl}
                partnerId={partnerId}
                currentUserId={currentUserId}
              />
            )}
          </div>
        </div>

        <MessageList
          channelId={conversation.id}
          members={members}
          channels={channels}
          onThreadOpen={onOpenThread}
          onSave={onSave}
          onForward={onForward}
          highlightMessageId={highlightMessageId}
          onTargetMessageFound={onTargetMessageFound}
        />

        <TypingIndicator channelId={conversation.id} currentUserId={currentUserId} />

        <MessageInput
          key={`conversation:${conversation.id}`}
          channelName={title}
          draftKey={`conversation:${conversation.id}`}
          isDm
          members={members}
          channels={channels}
          onSend={handleSend}
          attachments={attachments}
          onTyping={signalTyping}
          scheduleTarget={{ channelId: conversation.id }}
          pollTarget={{ channelId: conversation.id, instanceUrl }}
          workspaceId={workspaceId}
          instanceUrl={instanceUrl}
        />
      </div>
    </FileDropZone>
  );
}
