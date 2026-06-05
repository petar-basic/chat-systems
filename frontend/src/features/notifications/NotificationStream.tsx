import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import { globalEventBus } from '@/lib/globalEventBus';
import { useWorkspaceStore } from '@/stores/workspace';
import { useNotificationPrefs } from '@/stores/notificationPrefs';
import { showNotification, playNotificationSound } from '@/lib/notifications';
import { QUERY_KEYS, ROUTES, NOTIFICATION_SOUND_THROTTLE_MS } from '@/shared/constants';
import { NotificationType } from '@/models/enums';

let lastSoundAt = 0;

export function NotificationStream() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  useEffect(() => {
    return globalEventBus.on('notification', (event) => {
      const { workspace_id, channel_id, message_id, title, body, priority } = event;
      const isMention = priority === NotificationType.mention;

      if (channel_id) {
        useWorkspaceStore.setState((s) => {
          const nextUnread = new Set(s.unreadChannels);
          nextUnread.add(channel_id);
          const nextMention = new Set(s.mentionChannels);
          if (isMention) nextMention.add(channel_id);
          return { unreadChannels: nextUnread, mentionChannels: nextMention };
        });
      }

      const wsId = workspace_id ?? useWorkspaceStore.getState().currentWorkspace?.id;
      if (wsId) {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.notifications(wsId) });
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.notificationUnreadCount(wsId) });
      }

      const { soundEnabled, desktopEnabled } = useNotificationPrefs.getState();

      const now = Date.now();
      if (soundEnabled && !document.hasFocus() && now - lastSoundAt > NOTIFICATION_SOUND_THROTTLE_MS) {
        lastSoundAt = now;
        playNotificationSound();
      }

      if (desktopEnabled) {
        const onClick =
          wsId && channel_id
            ? () =>
                navigate(
                  message_id
                    ? ROUTES.message(wsId, channel_id, message_id)
                    : ROUTES.channel(wsId, channel_id),
                )
            : undefined;
        showNotification(isMention ? `🔔 ${title}` : title, body, onClick);
      }
    });
  }, [navigate, queryClient]);

  return null;
}
