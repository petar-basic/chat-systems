import { useMemo } from 'react';
import {
  useEditMessage,
  useDeleteMessage,
  useReactToMessage,
  useRemoveReaction,
  usePinMessage,
  useSendMessage,
} from '@/hooks/queries/useMessages';
import { useCurrentUser } from '@/hooks/queries/useAuth';
import { useWorkspaceStore } from '@/stores/workspace';
import { toast } from '@/shared/components/Toast';
import { ROUTES, ActionLabels } from '@/shared/constants';
import { logger } from '@/lib/logger';

export interface MessageActions {
  currentUserId: string;
  toggleReaction: (messageId: string, emoji: string, hasOwn: boolean) => void;
  togglePin: (messageId: string, isPinned: boolean) => void;
  editMessage: (messageId: string, content: string) => Promise<unknown>;
  deleteMessage: (messageId: string) => Promise<unknown>;
  retryMessage: (messageId: string, content: string) => void;
  copyLink: (messageId: string) => void;
}

export function useMessageActions(channelId: string): MessageActions {
  const { data: currentUser } = useCurrentUser();
  const userId = currentUser?.id ?? '';

  const edit = useEditMessage();
  const del = useDeleteMessage();
  const react = useReactToMessage();
  const removeReaction = useRemoveReaction();
  const pin = usePinMessage();
  const send = useSendMessage(channelId, userId);

  return useMemo(
    () => ({
      currentUserId: userId,
      toggleReaction: (messageId, emoji, hasOwn) =>
        hasOwn
          ? removeReaction.mutate({ messageId, channelId, emoji, userId })
          : react.mutate({ messageId, channelId, emoji, userId }),
      togglePin: (messageId, isPinned) => pin.mutate({ messageId, channelId, isPinned }),
      editMessage: (messageId, content) => edit.mutateAsync({ messageId, content, channelId }),
      deleteMessage: (messageId) => del.mutateAsync({ messageId, channelId }),
      retryMessage: (messageId, content) => send.mutate({ content, id: messageId }),
      copyLink: (messageId) => {
        const ws = useWorkspaceStore.getState().currentWorkspace;
        if (!ws) return;
        const url = `${window.location.origin}${ROUTES.message(ws.id, channelId, messageId)}`;
        navigator.clipboard
          ?.writeText(url)
          .then(() => toast.success(ActionLabels.LinkCopied))
          .catch((e) => logger.error('useMessageActions', 'copyLink', e));
      },
    }),
    [channelId, userId, react, removeReaction, pin, edit, del, send],
  );
}
