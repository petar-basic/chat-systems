import { queryClient } from './queryClient';
import { logger } from './logger';
import { QUERY_KEYS } from '@/shared/constants';

export function backfillAfterReconnect() {
  logger.info(
    'realtimeBackfill',
    'backfillAfterReconnect',
    'invalidating messages/notifications/conversations',
  );
  queryClient.invalidateQueries({ queryKey: QUERY_KEYS.messagesAll() });
  queryClient.invalidateQueries({ queryKey: QUERY_KEYS.notificationsAll() });
  queryClient.invalidateQueries({ queryKey: QUERY_KEYS.conversationsAll() });
  queryClient.invalidateQueries({ queryKey: QUERY_KEYS.huddlesActive() });
}
