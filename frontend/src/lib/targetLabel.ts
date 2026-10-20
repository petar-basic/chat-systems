import type { Channel } from '@/stores/workspace';
import type { Conversation } from '@/hooks/queries/useConversations';
import { conversationTitle } from './conversationHelpers';

/// Names the channel a message lives in, whether that is a channel with a name
/// or a conversation named after its participants.
export function targetLabel(
  channelId: string,
  channels: Channel[],
  conversations: Conversation[],
  currentUserId: string | undefined,
  nameOf: (userId: string) => string | undefined,
): string {
  const channel = channels.find((c) => c.id === channelId);
  if (channel) return `#${channel.name}`;
  const conversation = conversations.find((c) => c.id === channelId);
  return conversation ? conversationTitle(conversation, currentUserId, nameOf) : 'a channel you left';
}
