import { useCallback, useState } from 'react';
import type { Channel, Message } from '@/stores/workspace';
import type { Conversation } from '@/hooks/queries/useConversations';
import { useSaveMessage } from '@/hooks/queries/useSaved';
import type { ForwardSource } from '@/features/messaging/ForwardMessageModal';
import { targetLabel } from '@/lib/targetLabel';
import { toUserMessage } from '@/lib/errors';
import { toast } from '@/shared/components/Toast';

interface Args {
  workspaceId: string | undefined;
  currentWsInstanceUrl: string | undefined;
  channels: Channel[];
  conversations: Conversation[];
  userId: string | undefined;
  getUser: (id: string) => { display_name: string } | undefined;
}

/// Saving and forwarding; a direct message is a message in a channel nobody
/// can browse, so the only difference is how its origin is named.
export function useMessageActions({
  workspaceId,
  currentWsInstanceUrl,
  channels,
  conversations,
  userId,
  getUser,
}: Args) {
  const saveMessage = useSaveMessage(workspaceId ?? '', currentWsInstanceUrl);
  const [forwarding, setForwarding] = useState<ForwardSource | null>(null);

  const handleSaveMessage = useCallback(
    (message: Message) => {
      saveMessage.mutate(
        { messageId: message.id },
        {
          onSuccess: () => toast.success('Saved'),
          onError: (err) => toast.error(toUserMessage(err)),
        },
      );
    },
    [saveMessage],
  );

  const authorNameOf = useCallback((id: string) => getUser(id)?.display_name || 'Somebody', [getUser]);

  const handleForwardMessage = useCallback(
    (message: Message) => {
      setForwarding({
        content: message.content,
        authorName: authorNameOf(message.user_id),
        origin: targetLabel(
          message.channel_id,
          channels,
          conversations,
          userId,
          (id) => getUser(id)?.display_name,
        ),
      });
    },
    [authorNameOf, channels, conversations, getUser, userId],
  );

  return {
    handleSaveMessage,
    handleForwardMessage,
    forwarding,
    dismissForward: () => setForwarding(null),
  };
}
