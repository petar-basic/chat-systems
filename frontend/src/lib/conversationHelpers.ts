import type { Conversation } from '@/features/messaging/hooks/useConversations';
import i18n from '@/lib/i18n';
import { displayNameOf } from './userHelpers';

export function otherParticipants(conversation: Conversation, currentUserId: string | undefined) {
  const others = conversation.participant_ids.filter((id) => id !== currentUserId);
  return others.length > 0 ? others : conversation.participant_ids;
}

export function departedPartner(conversation: Conversation, currentUserId: string | undefined) {
  if (conversation.kind !== 'direct') return null;
  if (conversation.participant_ids.some((id) => id !== currentUserId)) return null;
  return conversation.departed[0] ?? null;
}

export function conversationTitle(
  conversation: Conversation,
  currentUserId: string | undefined,
  nameOf: (userId: string) => string | null | undefined,
): string {
  const present = conversation.participant_ids.filter((id) => id !== currentUserId);
  const departed = conversation.departed.map((p) => displayNameOf(p.display_name));
  const names =
    present.length + departed.length > 0
      ? [...present.map((id) => displayNameOf(nameOf(id))), ...departed]
      : conversation.participant_ids.map((id) => displayNameOf(nameOf(id)));
  const separator = i18n.t('common:listSeparator');
  if (names.length <= 3) return names.join(separator);
  return `${names.slice(0, 3).join(separator)} +${names.length - 3}`;
}

export function departureNote(conversation: Conversation): string | null {
  const names = conversation.departed.map((p) => displayNameOf(p.display_name));
  if (names.length === 0) return null;
  if (names.length === 1) return i18n.t('messages:conversation.departed.one', { name: names[0] });
  return i18n.t('messages:conversation.departed.many', {
    names: names.slice(0, -1).join(i18n.t('messages:conversation.departed.separator')),
    last: names[names.length - 1],
  });
}
