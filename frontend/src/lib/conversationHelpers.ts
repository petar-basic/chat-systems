import type { Conversation } from '@/features/messaging/hooks/useConversations';
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
  if (names.length <= 3) return names.join(', ');
  return `${names.slice(0, 3).join(', ')} +${names.length - 3}`;
}

export function departureNote(conversation: Conversation): string | null {
  const names = conversation.departed.map((p) => displayNameOf(p.display_name));
  if (names.length === 0) return null;
  if (names.length === 1) return `${names[0]} is no longer in this workspace`;
  return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]} are no longer in this workspace`;
}

export function isUnread(conversation: Conversation): boolean {
  return !conversation.last_read_at || conversation.last_message_at > conversation.last_read_at;
}
