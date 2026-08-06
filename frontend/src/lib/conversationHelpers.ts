import type { Conversation } from '@/hooks/queries/useConversations';
import { displayNameOf } from './userHelpers';

export function otherParticipants(conversation: Conversation, currentUserId: string | undefined) {
  const others = conversation.participant_ids.filter((id) => id !== currentUserId);
  return others.length > 0 ? others : conversation.participant_ids;
}

export function conversationTitle(
  conversation: Conversation,
  currentUserId: string | undefined,
  nameOf: (userId: string) => string | null | undefined,
): string {
  const names = otherParticipants(conversation, currentUserId).map((id) => displayNameOf(nameOf(id)));
  if (names.length <= 3) return names.join(', ');
  return `${names.slice(0, 3).join(', ')} +${names.length - 3}`;
}

export function isUnread(conversation: Conversation): boolean {
  return !conversation.last_read_at || conversation.last_message_at > conversation.last_read_at;
}
