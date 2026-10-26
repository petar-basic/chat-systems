import type { NotifyLevel } from '@/features/notifications/hooks/useNotifyLevels';

export const NOTIFY_LEVEL_LABELS: Record<NotifyLevel, string> = {
  all: 'All new messages',
  mentions: 'Mentions',
  nothing: 'Nothing',
};
