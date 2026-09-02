import type { Conversation } from '@/hooks/queries/useConversations';
import { conversationTitle } from '@/lib/conversationHelpers';
import { displayNameOf } from '@/lib/userHelpers';
import { useUserCache } from '@/stores/users';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import PresenceDot from '@/components/PresenceDot';

function UserAvatarWithPresence({
  userId,
  name,
  avatarUrl,
}: {
  userId: string;
  name: string;
  avatarUrl: string | null | undefined;
}) {
  return (
    <div className="relative shrink-0">
      <Avatar userId={userId} name={name} avatarUrl={avatarUrl} size="xs" />
      <PresenceDot userId={userId} className="absolute -bottom-0.5 -right-0.5 w-2 h-2 ring-2 ring-surface" />
    </div>
  );
}

export function ConversationRow({
  conversation,
  currentUserId,
  isActive,
  isUnread,
  onSelect,
}: {
  conversation: Conversation;
  currentUserId: string | undefined;
  isActive: boolean;
  isUnread: boolean;
  onSelect: (conversationId: string) => void;
}) {
  const { getUser } = useUserCache();
  const title = conversationTitle(conversation, currentUserId, (id) => getUser(id)?.display_name);
  const others = conversation.participant_ids.filter((id) => id !== currentUserId);
  const partnerId = others[0] ?? currentUserId ?? '';

  return (
    <button
      onClick={() => onSelect(conversation.id)}
      data-qa="conversation-row"
      data-conversation-id={conversation.id}
      className={`w-full px-3 py-1.5 flex items-center gap-2 text-sm transition cursor-pointer ${
        isActive
          ? 'bg-purple-600/20 text-fg'
          : isUnread
            ? 'text-fg font-semibold hover:bg-raised/30'
            : 'text-muted hover:bg-raised/30 hover:text-fg-soft'
      }`}
    >
      {conversation.kind === 'group' ? (
        <span className="relative shrink-0 flex -space-x-2">
          {others.slice(0, 2).map((id) => (
            <Avatar
              key={id}
              userId={id}
              name={displayNameOf(getUser(id)?.display_name)}
              avatarUrl={getUser(id)?.avatar_url}
              size="xs"
              className="ring-2 ring-surface"
            />
          ))}
        </span>
      ) : (
        <UserAvatarWithPresence userId={partnerId} name={title} avatarUrl={getUser(partnerId)?.avatar_url} />
      )}
      <span className="truncate">{title}</span>
      {isUnread && !isActive && <span className="ml-auto w-2 h-2 bg-purple-400 rounded-full shrink-0" />}
    </button>
  );
}

export function MemberRow({ userId, onOpenDm }: { userId: string; onOpenDm: (id: string) => void }) {
  const { getUser } = useUserCache();
  const cached = getUser(userId);

  const name = displayNameOf(cached?.display_name);

  return (
    <button
      onClick={() => onOpenDm(userId)}
      className="w-full px-3 py-1 flex items-center gap-2 text-sm text-muted hover:bg-raised/30 hover:text-fg-soft transition cursor-pointer"
      title={cached?.status_text ? `Message ${name} — ${cached.status_text}` : `Message ${name}`}
    >
      <UserAvatarWithPresence userId={userId} name={name} avatarUrl={cached?.avatar_url} />
      <span className="truncate">{name}</span>
      {cached?.status_emoji && (
        <span className="shrink-0" data-qa="member-status-emoji" title={cached.status_text ?? undefined}>
          {cached.status_emoji}
        </span>
      )}
    </button>
  );
}
