import { useEffect } from 'react';
import { useNavigate } from 'react-router';
import { useQueryClient } from '@tanstack/react-query';
import { globalEventBus } from '@/lib/globalEventBus';
import { useWorkspaceStore } from '@/stores/workspace';
import { useNotificationPrefs } from '@/stores/notificationPrefs';
import { showNotification, playMessageSound } from '@/lib/notifications';
import { QUERY_KEYS, ROUTES } from '@/shared/constants';
import { useOpenMessage } from '@/features/messaging';
import { NotificationType } from '@/shared/models/enums';

export function NotificationStream() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const openMessage = useOpenMessage();

  useEffect(() => {
    return globalEventBus.on('notification', (event) => {
      const { workspace_id, channel_id, message_id, thread_parent_id, list_id, item_id, title, body, kind } =
        event;
      const isMention = kind === NotificationType.mention;
      const isReply = kind === NotificationType.reply;
      const isList = kind === NotificationType.list;

      if (channel_id && isMention) {
        useWorkspaceStore.setState((s) => {
          const nextUnread = new Set(s.unreadChannels);
          nextUnread.add(channel_id);
          const nextMention = new Set(s.mentionChannels);
          nextMention.add(channel_id);
          return { unreadChannels: nextUnread, mentionChannels: nextMention };
        });
      }

      const wsId = workspace_id ?? useWorkspaceStore.getState().currentWorkspace?.id;
      if (wsId) {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.notifications(wsId) });
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.notificationUnreadCount(wsId) });
      }

      const { desktopEnabled } = useNotificationPrefs.getState();
      const { currentChannel, currentConversationId } = useWorkspaceStore.getState();
      const viewing =
        !!channel_id &&
        !isReply &&
        !isList &&
        (currentChannel?.id === channel_id || currentConversationId === channel_id) &&
        document.hasFocus();
      if (viewing) return;
      playMessageSound();

      if (desktopEnabled) {
        const onClick =
          wsId && isList && list_id
            ? () => navigate(`${ROUTES.list(wsId, list_id)}${item_id ? `?item=${item_id}` : ''}`)
            : message_id
              ? () => void openMessage(message_id, !!thread_parent_id)
              : wsId && channel_id
                ? () => navigate(ROUTES.channel(wsId, channel_id))
                : undefined;
        showNotification(isMention ? `🔔 ${title}` : title, body ?? '', onClick);
      }
    });
  }, [navigate, queryClient, openMessage]);

  return null;
}
