import type { TFunction } from 'i18next';
import type { components } from '@/api/schema';
import { failureKey } from '@/lib/scheduledFailure';

export type NotificationData = components['schemas']['NotificationData'];

export interface NotificationText {
  title: string;
  body: string;
}

type Translate = TFunction<['notifications', 'messages']>;

function systemText(t: Translate, data: NotificationData, said: string, sender: string): NotificationText {
  switch (data.notice) {
    case 'workflow_stopped':
      return {
        title: t('activity.workflowStopped'),
        body: data.stopped
          ? t('activity.workflowStoppedBody', {
              workflow: data.workflow_name ?? '',
              reason: t(`activity.workflowStops.${data.stopped}`),
            })
          : said,
      };
    case 'app_requested':
      return { title: t('activity.appRequested', { sender }), body: said };
    case 'not_delivered':
      return {
        title: t('activity.notDelivered'),
        body: t(`messages:scheduled.reasons.${failureKey(data.failure)}`),
      };
    default:
      return { title: t('activity.notice'), body: said };
  }
}

export function notificationText(
  t: Translate,
  kind: string | null | undefined,
  data: NotificationData,
  body: string | null | undefined,
): NotificationText {
  const sender = data.sender_name || t('activity.someone');
  const channel = data.channel_name ?? '';
  const keyword = data.keyword ?? '';
  const said = body ?? '';
  switch (kind) {
    case 'mention':
      return { title: t('activity.mention'), body: said };
    case 'reply':
      return { title: t('activity.reply'), body: said };
    case 'dm':
      return { title: sender, body: said };
    case 'keyword':
      return {
        title: channel ? t('activity.keywordIn', { keyword, channel }) : t('activity.keyword', { keyword }),
        body: said,
      };
    case 'list': {
      const item = data.item_title || t('activity.untitled');
      return data.assigned
        ? {
            title: t('activity.listAssigned', { sender, item }),
            body: t('activity.listIn', { list: data.list_title ?? '' }),
          }
        : { title: t('activity.listMentioned', { sender, item }), body: said };
    }
    case 'call':
      return { title: t('activity.call'), body: t('activity.callBody') };
    case 'reminder':
      return { title: t('activity.reminder'), body: said || t('activity.reminderBody') };
    case 'system':
      return systemText(t, data, said, sender);
    default:
      return { title: t('activity.message', { sender, channel }), body: said };
  }
}
