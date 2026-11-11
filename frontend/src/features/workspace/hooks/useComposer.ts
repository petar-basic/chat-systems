import { useCallback } from 'react';
import { asAttachments, useComposerAttachments, useSendMessage, useSlashCommand } from '@/features/messaging';
import type { Channel, Workspace } from '@/stores/workspace';
import { useTypingSignal, typingTargetOf } from '@/shared/hooks/useTypingSignal';

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

  const command = useSlashCommand(currentChannel?.id, undefined, currentWsInstanceUrl);

  const handleSend = useCallback(
    async (content: string) => {
      if (!currentChannel || !userId) return;
      stopTyping();
      if (await command.run(content)) return;

      const id = crypto.randomUUID();
      sendMessageMutation.mutate({ content, id, attachments: asAttachments(attachments.take()) });
    },
    [currentChannel, userId, stopTyping, command, sendMessageMutation, attachments],
  );

  return {
    ephemeral: command.answer,
    ephemeralPending: command.pending,
    dismissEphemeral: command.dismiss,
    handleTyping: signalTyping,
    handleSend,
    attachments,
  };
}
