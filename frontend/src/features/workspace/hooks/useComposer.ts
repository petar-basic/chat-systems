import { useCallback, useEffect, useState } from 'react';
import { asAttachments, useComposerAttachments } from '@/features/messaging/useComposerAttachments';
import type { Channel, Workspace } from '@/stores/workspace';
import { commandResultText, parseCommand, runCommand } from '@/lib/slashCommands';
import { toUserMessage } from '@/lib/errors';
import { useTypingSignal, typingTargetOf } from '@/shared/hooks/useTypingSignal';
import { useSendMessage } from '@/features/messaging/hooks/useMessages';

interface Args {
  currentWorkspace: Workspace | null;
  currentChannel: Channel | null;
  userId: string | undefined;
  currentWsInstanceUrl: string | undefined;
}

/// Sending into the open channel: slash commands, plain messages, attachments,
/// and the typing signal that goes with them.
export function useComposer({ currentWorkspace, currentChannel, userId, currentWsInstanceUrl }: Args) {
  const sendMessageMutation = useSendMessage(currentChannel?.id ?? '', userId ?? '');
  const attachments = useComposerAttachments({
    workspaceId: currentChannel ? currentWorkspace?.id : undefined,
    instanceUrl: currentWorkspace?.instanceUrl,
    resetKey: currentChannel?.id,
  });
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
      sendMessageMutation.mutate({ content, id, attachments: asAttachments(attachments.take()) });
    },
    [currentChannel, userId, stopTyping, sendMessageMutation, currentWsInstanceUrl, attachments],
  );

  return {
    ephemeral,
    dismissEphemeral: () => setCommandBanner(null),
    handleTyping: signalTyping,
    handleSend,
    attachments,
  };
}
