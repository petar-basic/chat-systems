import { useCallback, useEffect } from 'react';
import { asAttachments, useComposerAttachments, useSendMessage } from '@/features/messaging';
import type { Channel, Workspace } from '@/stores/workspace';
import { commandResultText, parseCommand, runCommand } from '@/lib/slashCommands';
import { toUserMessage } from '@/lib/errors';
import { useTypingSignal, typingTargetOf } from '@/shared/hooks/useTypingSignal';
import { useCommandAnswerStore } from '@/stores/commandAnswer';

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

  const answer = useCommandAnswerStore((s) => s.answer);
  const showAnswer = useCommandAnswerStore((s) => s.show);
  const dismissAnswer = useCommandAnswerStore((s) => s.dismiss);
  const shown = answer && answer.channelId === currentChannel?.id ? answer : null;

  useEffect(() => {
    if (!shown || shown.pending) return undefined;
    const timer = setTimeout(dismissAnswer, 10_000);
    return () => clearTimeout(timer);
  }, [shown, dismissAnswer]);

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
            if (result.response_type === 'in_channel') dismissAnswer();
            else
              showAnswer({
                text: commandResultText(result),
                channelId: currentChannel.id,
                pending: result.response_type === 'pending',
              });
            return;
          }
        } catch (e) {
          showAnswer({ text: toUserMessage(e), channelId: currentChannel.id, pending: false });
          return;
        }
      }

      const id = crypto.randomUUID();
      sendMessageMutation.mutate({ content, id, attachments: asAttachments(attachments.take()) });
    },
    [
      currentChannel,
      userId,
      stopTyping,
      sendMessageMutation,
      currentWsInstanceUrl,
      attachments,
      showAnswer,
      dismissAnswer,
    ],
  );

  return {
    ephemeral: shown?.text ?? null,
    ephemeralPending: shown?.pending ?? false,
    dismissEphemeral: dismissAnswer,
    handleTyping: signalTyping,
    handleSend,
    attachments,
  };
}
