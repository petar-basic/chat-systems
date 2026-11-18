import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { globalEventBus } from './globalEventBus';
import { QUERY_KEYS } from '@/shared/constants';
import { syncAppFrames } from './appSync';

const home = {
  id: 'home-1',
  app_id: 'app-1',
  hash: 'h2',
  type: 'home' as const,
  blocks: [{ type: 'header' as const, text: { type: 'plain_text' as const, text: 'Your requests' } }],
};

describe('syncAppFrames', () => {
  let queryClient: QueryClient;
  let unsubs: Array<() => void>;

  const stale = (key: readonly unknown[]) => queryClient.getQueryState(key)?.isInvalidated;

  beforeEach(() => {
    queryClient = new QueryClient();
    unsubs = syncAppFrames(queryClient);
    queryClient.setQueryData(QUERY_KEYS.activeApps('w-1'), []);
    queryClient.setQueryData(QUERY_KEYS.activeApps('w-2'), []);
    queryClient.setQueryData(QUERY_KEYS.messageShortcuts('ch-1'), []);
    queryClient.setQueryData(QUERY_KEYS.channelMembers('ch-1'), []);
    queryClient.setQueryData(QUERY_KEYS.appHome('app-1'), { app_id: 'app-1', home: null });
  });

  afterEach(() => {
    unsubs.forEach((unsub) => unsub());
    queryClient.clear();
  });

  it('refetches the apps, their shortcuts and Homes when the apps change', () => {
    globalEventBus.emit({ type: 'workspace.apps_changed', workspace_id: 'w-1' });

    expect(stale(QUERY_KEYS.activeApps('w-1'))).toBe(true);
    expect(stale(QUERY_KEYS.messageShortcuts('ch-1'))).toBe(true);
    expect(stale(QUERY_KEYS.appHome('app-1'))).toBe(true);
    expect(stale(QUERY_KEYS.activeApps('w-2'))).toBe(false);
    expect(stale(QUERY_KEYS.channelMembers('ch-1'))).toBe(false);
  });

  it('shows a republished Home without asking for it', () => {
    globalEventBus.emit({ type: 'views.home_changed', workspace_id: 'w-1', app_id: 'app-1', home });

    expect(queryClient.getQueryData(QUERY_KEYS.appHome('app-1'))).toEqual({ app_id: 'app-1', home });
  });
});
