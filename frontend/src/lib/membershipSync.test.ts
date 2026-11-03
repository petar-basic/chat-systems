import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { globalEventBus } from './globalEventBus';
import { QUERY_KEYS } from '@/shared/constants';
import { syncMembershipFrames } from './membershipSync';

describe('syncMembershipFrames', () => {
  let queryClient: QueryClient;
  let unsubs: Array<() => void>;

  const stale = (key: readonly unknown[]) => queryClient.getQueryState(key)?.isInvalidated;

  beforeEach(() => {
    queryClient = new QueryClient();
    unsubs = syncMembershipFrames(queryClient);
    queryClient.setQueryData(QUERY_KEYS.workspacesList('https://chat.example'), []);
    queryClient.setQueryData(QUERY_KEYS.workspaceMembers('w-1'), []);
    queryClient.setQueryData(QUERY_KEYS.workspaceMembers('w-2'), []);
    queryClient.setQueryData(QUERY_KEYS.channelMembers('ch-1'), []);
    queryClient.setQueryData(QUERY_KEYS.messages('ch-1'), { pages: [], pageParams: [] });
  });

  afterEach(() => {
    unsubs.forEach((unsub) => unsub());
    queryClient.clear();
  });

  it('refetches the workspace list when this user is added to a workspace', () => {
    globalEventBus.emit({ type: 'workspace.joined', workspace_id: 'w-3' });

    expect(stale(QUERY_KEYS.workspacesList('https://chat.example'))).toBe(true);
  });

  it('refetches the member lists when someone joins or leaves', () => {
    globalEventBus.emit({ type: 'workspace.members_changed', workspace_id: 'w-1' });

    expect(stale(QUERY_KEYS.workspaceMembers('w-1'))).toBe(true);
    expect(stale(QUERY_KEYS.channelMembers('ch-1'))).toBe(true);
    expect(stale(QUERY_KEYS.workspaceMembers('w-2'))).toBe(false);
    expect(stale(QUERY_KEYS.messages('ch-1'))).toBe(false);
  });
});
