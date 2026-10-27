import { queryClient } from './queryClient';
import { logger } from './logger';
import { QUERY_KEYS } from '@/shared/constants';

const OPEN_VIEWS = [
  QUERY_KEYS.messagesAll(),
  QUERY_KEYS.notificationsAll(),
  QUERY_KEYS.conversationsAll(),
  QUERY_KEYS.huddlesActive(),
];

const COUNTED_BY_EVENTS = [
  QUERY_KEYS.threadsAll(),
  QUERY_KEYS.threadStateAll(),
  QUERY_KEYS.followedThreadsAll(),
  QUERY_KEYS.channels(),
  QUERY_KEYS.unreadsAll(),
  QUERY_KEYS.sidebarAll(),
  QUERY_KEYS.laterAll(),
];

function invalidate(keys: ReadonlyArray<readonly string[]>) {
  for (const queryKey of keys) queryClient.invalidateQueries({ queryKey });
}

export function backfillAfterReconnect() {
  logger.info(
    'realtimeBackfill',
    'backfillAfterReconnect',
    'invalidating messages/notifications/conversations',
  );
  invalidate(OPEN_VIEWS);
}

export function refetchAfterGap() {
  logger.info('realtimeBackfill', 'refetchAfterGap', 'invalidating everything a missed event could change');
  invalidate([...OPEN_VIEWS, ...COUNTED_BY_EVENTS]);
}
