import type { NotifyLevel } from '@/hooks/queries/useNotifyLevels';

export const NOTIFY_LEVEL_LABELS: Record<NotifyLevel, string> = {
  all: 'All new messages',
  mentions: 'Mentions',
  nothing: 'Nothing',
};
