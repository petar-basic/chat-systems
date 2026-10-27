import { describe, it, expect, afterEach } from 'vitest';
import { queryClient } from './queryClient';
import { backfillAfterReconnect, refetchAfterGap } from './realtimeBackfill';
import { QUERY_KEYS } from '@/shared/constants';

describe('realtimeBackfill', () => {
  afterEach(() => {
    queryClient.clear();
  });

  it('after a gap too long to replay, marks everything a missed event could have changed as stale', () => {
    const missed = [
      QUERY_KEYS.messages('c1'),
      QUERY_KEYS.channelPins('c1'),
      QUERY_KEYS.channelsUnread('w1'),
      QUERY_KEYS.unreads('w1'),
      QUERY_KEYS.sidebar('w1'),
      QUERY_KEYS.unreadThreads('w1'),
      QUERY_KEYS.threadState('m1'),
      QUERY_KEYS.notificationUnreadCount('w1'),
      QUERY_KEYS.later('w1'),
    ];
    for (const key of missed) queryClient.setQueryData(key, { cached: true });
    queryClient.setQueryData(QUERY_KEYS.currentUser(), { cached: true });

    refetchAfterGap();

    for (const key of missed) {
      expect(queryClient.getQueryState(key)?.isInvalidated, JSON.stringify(key)).toBe(true);
    }
    expect(queryClient.getQueryState(QUERY_KEYS.currentUser())?.isInvalidated).toBe(false);
  });

  it('after a replayed reconnect, leaves the counters to the replayed events', () => {
    queryClient.setQueryData(QUERY_KEYS.messages('c1'), { cached: true });
    queryClient.setQueryData(QUERY_KEYS.channelsUnread('w1'), { cached: true });

    backfillAfterReconnect();

    expect(queryClient.getQueryState(QUERY_KEYS.messages('c1'))?.isInvalidated).toBe(true);
    expect(
      queryClient.getQueryState(QUERY_KEYS.channelsUnread('w1'))?.isInvalidated,
      'a refetched count that already includes a replayed message would be bumped by it again',
    ).toBe(false);
  });
});
