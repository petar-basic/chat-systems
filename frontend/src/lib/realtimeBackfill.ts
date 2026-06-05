import { queryClient } from './queryClient';
import { logger } from './logger';
import { QUERY_KEYS } from '@/shared/constants';

export function backfillAfterReconnect() {
  logger.info('realtimeBackfill', 'backfillAfterReconnect', 'invalidating messages/notifications/dm');
  queryClient.invalidateQueries({ queryKey: QUERY_KEYS.messagesAll() });
  queryClient.invalidateQueries({ queryKey: QUERY_KEYS.notificationsAll() });
  queryClient.invalidateQueries({ queryKey: QUERY_KEYS.dm() });
}
