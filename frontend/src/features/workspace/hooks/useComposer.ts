import { useCallback, useEffect, useState } from 'react';
import { useAttachmentUpload } from '@/features/messaging/useAttachmentUpload';
import type { Channel, Workspace } from '@/stores/workspace';
import { commandResultText, parseCommand, runCommand } from '@/lib/slashCommands';
import { toUserMessage } from '@/lib/errors';
import { useTypingSignal, typingTargetOf } from '@/shared/hooks/useTypingSignal';
import { useSendMessage } from '@/hooks/queries/useMessages';

interface Args {
  currentWorkspace: Workspace | null;
  currentChannel: Channel | null;
  userId: string | undefined;
  currentWsInstanceUrl: string | undefined;
}

/// Sending into the open channel: slash commands, plain messages, file uploads,
/// and the typing signal that goes with them.
export function useComposer({ currentWorkspace, currentChannel, userId, currentWsInstanceUrl }: Args) {
  const sendMessageMutation = useSendMessage(currentChannel?.id ?? '', userId ?? '');
  const { signalTyping, stopTyping } = useTypingSignal(
    typingTargetOf(currentChannel?.id),
    currentWorkspace?.instanceUrl,
  );

  const [commandBanner, setCommandBanner] = useState<{ text: string; channelId: string } | null>(null);
  const ephemeral = commandBanner?.channelId === currentChannel?.id ? (commandBanner?.text ?? null) : null;

  useEffect(() => {
    if (!ephemeral) return undefined;
    const timer = setTimeout(() => setCommandBanner(null), 10_000);
    return () => clearTimeout(timer);
  }, [ephemeral]);

  const handleSend = useCallback(
    async (content: string) => {
      if (!currentChannel || !userId) return;
      stopTyping();

      if (parseCommand(content)) {
        try {
          const result = await runCommand(currentChannel.id, content, currentWsInstanceUrl);
          // An unknown command falls through to being sent as text; anything
          // else has already done its work on the server.
          if (result) {
            setCommandBanner(
              result.response_type === 'ephemeral'
                ? { text: commandResultText(result), channelId: currentChannel.id }
                : null,
            );
            return;
          }
        } catch (e) {
          setCommandBanner({ text: toUserMessage(e), channelId: currentChannel.id });
          return;
        }
      }

      const id = crypto.randomUUID();
      sendMessageMutation.mutate({ content, id });
    },
    [currentChannel, userId, stopTyping, sendMessageMutation, currentWsInstanceUrl],
  );

  const { uploading, handleFileUpload } = useAttachmentUpload({
    workspaceId: currentChannel ? currentWorkspace?.id : undefined,
    instanceUrl: currentWorkspace?.instanceUrl,
    send: (content) => sendMessageMutation.mutate({ content, id: crypto.randomUUID() }),
  });

  return {
    ephemeral,
    dismissEphemeral: () => setCommandBanner(null),
    handleTyping: signalTyping,
    handleSend,
    handleFileUpload,
    uploading,
  };
}
