import { queryClient } from './queryClient';
import { logger } from './logger';
import { QUERY_KEYS } from '@/shared/constants';

const MISSED_BY_A_GAP = [
  QUERY_KEYS.messagesAll(),
  QUERY_KEYS.threadsAll(),
  QUERY_KEYS.threadStateAll(),
  QUERY_KEYS.followedThreadsAll(),
  QUERY_KEYS.notificationsAll(),
  QUERY_KEYS.conversationsAll(),
  QUERY_KEYS.channels(),
  QUERY_KEYS.unreadsAll(),
  QUERY_KEYS.sidebarAll(),
  QUERY_KEYS.laterAll(),
  QUERY_KEYS.huddlesActive(),
];

export function backfillAfterReconnect() {
  logger.info('realtimeBackfill', 'backfillAfterReconnect', 'invalidating what a gap could have changed');
  for (const queryKey of MISSED_BY_A_GAP) queryClient.invalidateQueries({ queryKey });
}
