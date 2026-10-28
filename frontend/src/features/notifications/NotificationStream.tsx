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
import { onScreen, readOnScreen } from '@/lib/readOnScreen';

export function NotificationStream() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const openMessage = useOpenMessage();

  useEffect(() => {
    return globalEventBus.on('notification', (event, instanceUrl) => {
      const { workspace_id, channel_id, message_id, thread_parent_id, list_id, item_id, title, body, kind } =
        event;
      const isMention = kind === NotificationType.mention;
      const isReply = kind === NotificationType.reply;
      const isList = kind === NotificationType.list;
      const here = !workspace_id || workspace_id === useWorkspaceStore.getState().currentWorkspace?.id;

      if (here && channel_id && isMention && !thread_parent_id) {
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

      if (here && thread_parent_id && document.hasFocus()) {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.threadState(thread_parent_id) });
      }

      const { desktopEnabled } = useNotificationPrefs.getState();
      const viewing =
        here && !!channel_id && !isReply && !isList && !thread_parent_id && !!onScreen(channel_id);
      if (viewing) {
        readOnScreen(queryClient, channel_id);
        return;
      }
      playMessageSound();

      if (desktopEnabled) {
        const onClick =
          wsId && isList && list_id
            ? () => navigate(`${ROUTES.list(wsId, list_id)}${item_id ? `?item=${item_id}` : ''}`)
            : message_id
              ? () => void openMessage(message_id, !!thread_parent_id, instanceUrl)
              : wsId && channel_id
                ? () => navigate(ROUTES.channel(wsId, channel_id))
                : undefined;
        showNotification(
          isMention ? `🔔 ${title}` : title,
          body ?? '',
          channel_id ?? 'chat-systems',
          onClick,
        );
      }
    });
  }, [navigate, queryClient, openMessage]);

  return null;
}
