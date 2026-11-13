import { useState } from 'react';
import { ArrowLeft, Hash, Menu, UserMinus, UserPlus } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useUserCache } from '@/stores/users';
import { usePresenceStore } from '@/stores/presence';
import type { Channel, Message, WorkspaceMember } from '@/stores/workspace';
import type { Conversation } from '@/features/messaging/hooks/useConversations';
import { useSendMessage } from '@/features/messaging/hooks/useMessages';
import { conversationTitle, departedPartner, departureNote } from '@/lib/conversationHelpers';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { ConnectionBanner } from '@/shared/components/ConnectionBanner/ConnectionBanner';
import { HuddleStartButton } from '@/features/huddle';
import { MAX_GROUP_OTHERS, NewDmModal } from '@/features/channel';
import { NotificationLevelButton } from '@/features/notifications';
import { useTypingSignal, typingTargetOf } from '@/shared/hooks/useTypingSignal';
import { FileDropZone } from '@/shared/components/FileDropZone';
import PresenceDot from '@/shared/components/PresenceDot';
import TypingIndicator from '@/features/messaging/TypingIndicator';
import MessageList from './MessageList';
import MessageInput from './MessageInput';
import { OffHoursNote } from './OffHoursNote';
import { asAttachments, useComposerAttachments } from './useComposerAttachments';
import { useSlashCommand } from './hooks/useSlashCommand';
import { CommandAnswer } from './CommandAnswer';
import { ConvertConversationModal } from './ConvertConversationModal';

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
  onOpenThread: (message: Message, replyId?: string) => void;
  onSave: (message: Message) => void;
  onForward: (message: Message) => void;
  onAddPeople: (participantIds: string[]) => void;
  canConvert: boolean;
  onConverted: (channel: Channel) => void;
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
  onAddPeople,
  canConvert,
  onConverted,
}: Props) {
  const { t } = useTranslation(['messages', 'channel']);
  const { getUser } = useUserCache();
  const title = conversationTitle(conversation, currentUserId, (id) => getUser(id)?.display_name);
  const participantIds = conversation.participant_ids;
  const partnerId = participantIds.find((id) => id !== currentUserId) ?? currentUserId;
  const partner = getUser(partnerId);
  const status = usePresenceStore((s) => s.getStatus(partnerId));
  const isDirect = conversation.kind === 'direct';
  const gone = departedPartner(conversation, currentUserId);
  const note = departureNote(conversation);
  const others = participantIds.filter((id) => id !== currentUserId);
  const canAddPeople = !gone && others.length > 0 && others.length < MAX_GROUP_OTHERS;
  const [addingPeople, setAddingPeople] = useState(false);
  const convertible = canConvert && !isDirect && participantIds.length >= 3;
  const [converting, setConverting] = useState(false);

  const sendMutation = useSendMessage(conversation.id, currentUserId);
  const { signalTyping, stopTyping } = useTypingSignal(typingTargetOf(conversation.id), instanceUrl);
  const attachments = useComposerAttachments({ workspaceId, instanceUrl, resetKey: conversation.id });
  const command = useSlashCommand(conversation.id, undefined, instanceUrl);
  const handleSend = async (content: string) => {
    stopTyping();
    if (await command.run(content)) return;
    sendMutation.mutate({ content, id: crypto.randomUUID(), attachments: asAttachments(attachments.take()) });
  };

  return (
    <FileDropZone className="flex-1 flex min-w-0" disabled={!!gone} onFiles={attachments.add}>
      <div
        role="main"
        aria-label={t('conversation.label')}
        className="flex-1 flex flex-col min-w-0"
        data-message-pane
      >
        <ConnectionBanner instanceUrl={instanceUrl} />
        <div className="h-12 px-4 flex items-center gap-3 border-b border-line/50 shrink-0">
          {onOpenNav && (
            <button
              onClick={onOpenNav}
              aria-label={t('header.openNavigation', { ns: 'channel' })}
              data-tooltip={t('header.openNavigation', { ns: 'channel' })}
              className="lg:hidden p-1.5 -ml-1 rounded-lg text-muted hover:text-fg hover:bg-raised/50"
            >
              <Menu className="w-5 h-5" />
            </button>
          )}
          <button
            onClick={onClose}
            aria-label={t('conversation.back')}
            data-tooltip={t('conversation.back')}
            className="text-muted hover:text-fg transition cursor-pointer mr-1"
          >
            <ArrowLeft className="w-4 h-4" />
          </button>
          {gone ? (
            <Avatar userId={gone.user_id} name={title} avatarUrl={gone.avatar_url} size="sm" />
          ) : isDirect ? (
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
          {!isDirect && (
            <span className="text-xs text-muted shrink-0">
              {t('conversation.people', { count: participantIds.length })}
            </span>
          )}
          {isDirect && !gone && status === 'online' && (
            <span className="text-xs text-muted">{t('conversation.activeNow')}</span>
          )}
          <div className="ml-auto flex items-center gap-1">
            {canAddPeople && (
              <button
                onClick={() => setAddingPeople(true)}
                aria-label={t('conversation.addPeople')}
                data-tooltip={t('conversation.addPeople')}
                data-qa="conversation-add-people"
                className="p-1.5 rounded-lg text-muted hover:text-fg hover:bg-raised/50"
              >
                <UserPlus className="w-4 h-4" />
              </button>
            )}
            {convertible && (
              <button
                onClick={() => setConverting(true)}
                aria-label={t('conversation.convert.open')}
                data-tooltip={t('conversation.convert.open')}
                data-qa="conversation-convert"
                className="p-1.5 rounded-lg text-muted hover:text-fg hover:bg-raised/50"
              >
                <Hash className="w-4 h-4" />
              </button>
            )}
            <NotificationLevelButton channelId={conversation.id} direct />
            {isDirect && !gone && (
              <HuddleStartButton
                workspaceId={workspaceId}
                instanceUrl={instanceUrl}
                partnerId={partnerId}
                currentUserId={currentUserId}
              />
            )}
          </div>
        </div>

        <div className="relative flex min-h-0 flex-1 flex-col">
          <MessageList
            channelId={conversation.id}
            draftKey={`conversation:${conversation.id}`}
            markReadIn={workspaceId}
            members={members}
            channels={channels}
            onThreadOpen={onOpenThread}
            onSave={onSave}
            onForward={onForward}
            highlightMessageId={highlightMessageId}
            onTargetMessageFound={onTargetMessageFound}
          />

          <TypingIndicator channelId={conversation.id} currentUserId={currentUserId} />
        </div>
        {isDirect && partnerId !== currentUserId && <OffHoursNote userId={partnerId} />}

        {note && (
          <div
            data-qa="conversation-departed"
            className={
              gone
                ? 'mx-4 mb-4 px-4 py-3 rounded-lg bg-surface/60 border border-line text-sm text-muted flex items-center gap-2'
                : 'mx-4 mb-1 text-xs text-muted flex items-center gap-1.5'
            }
          >
            <UserMinus className={gone ? 'w-4 h-4 shrink-0' : 'w-3.5 h-3.5 shrink-0'} />
            {note}
          </div>
        )}

        <CommandAnswer text={command.answer} pending={command.pending} onDismiss={command.dismiss} />

        {!gone && (
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
            slashCommands
            workspaceId={workspaceId}
            instanceUrl={instanceUrl}
          />
        )}
      </div>
      {addingPeople && (
        <NewDmModal
          members={members}
          currentUserId={currentUserId}
          adding={others}
          onStart={onAddPeople}
          onClose={() => setAddingPeople(false)}
        />
      )}
      {converting && (
        <ConvertConversationModal
          conversationId={conversation.id}
          instanceUrl={instanceUrl}
          channels={channels}
          onConverted={onConverted}
          onClose={() => setConverting(false)}
        />
      )}
    </FileDropZone>
  );
}
