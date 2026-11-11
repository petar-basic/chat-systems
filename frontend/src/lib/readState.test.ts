import { beforeEach, describe, expect, it } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { applyReadState, recountAfterReplay } from './readState';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';
import { QUERY_KEYS } from '@/shared/constants';

const workspace = { id: 'ws-1' } as Workspace;

describe('applyReadState', () => {
  let queryClient: QueryClient;

  beforeEach(() => {
    queryClient = new QueryClient();
    useWorkspaceStore.setState({
      currentWorkspace: workspace,
      unreadChannels: new Set(),
      mentionChannels: new Set(),
      unreadCounts: {},
      mentionCounts: {},
      unreadConversations: new Set(),
    });
  });

  it('brings a channel back as unread with its counts', () => {
    applyReadState(queryClient, { channel_id: 'ch-1', unread_count: 3, mention_count: 1 });
    const s = useWorkspaceStore.getState();
    expect(s.unreadChannels.has('ch-1')).toBe(true);
    expect(s.unreadCounts['ch-1']).toBe(3);
    expect(s.mentionChannels.has('ch-1')).toBe(true);
    expect(s.mentionCounts['ch-1']).toBe(1);
  });

  it('clears a channel read on another device', () => {
    applyReadState(queryClient, { channel_id: 'ch-1', unread_count: 3, mention_count: 1 });
    applyReadState(queryClient, { channel_id: 'ch-1', unread_count: 0, mention_count: 0 });
    const s = useWorkspaceStore.getState();
    expect(s.unreadChannels.has('ch-1')).toBe(false);
    expect(s.unreadCounts['ch-1']).toBeUndefined();
    expect(s.mentionCounts['ch-1']).toBeUndefined();
  });

  it('marks a direct message through the conversation list', () => {
    queryClient.setQueryData(QUERY_KEYS.conversations('ws-1'), [{ id: 'dm-1' }]);
    applyReadState(queryClient, { channel_id: 'dm-1', unread_count: 2, mention_count: 0 });
    const s = useWorkspaceStore.getState();
    expect(s.unreadConversations.has('dm-1')).toBe(true);
    expect(s.unreadChannels.has('dm-1')).toBe(false);
  });

  it('keeps the conversation list on the count, so a refetch does not bring a read DM back', () => {
    queryClient.setQueryData(QUERY_KEYS.conversations('ws-1'), [
      { id: 'dm-1', unread_count: 2 },
      { id: 'dm-2', unread_count: 1 },
    ]);
    applyReadState(queryClient, { channel_id: 'dm-1', unread_count: 0, mention_count: 0 });
    expect(queryClient.getQueryData(QUERY_KEYS.conversations('ws-1'))).toEqual([
      { id: 'dm-1', unread_count: 0 },
      { id: 'dm-2', unread_count: 1 },
    ]);
    expect(useWorkspaceStore.getState().unreadConversations.has('dm-1')).toBe(false);
  });
});

describe('recountAfterReplay', () => {
  const counts = {
    channel_ids: ['ch-1'],
    counts: [{ channel_id: 'ch-1', unread_count: 3, mention_count: 1 }],
  };

  beforeEach(() => {
    useWorkspaceStore.setState({
      currentWorkspace: workspace,
      unreadChannels: new Set(),
      mentionChannels: new Set(),
      unreadCounts: {},
      mentionCounts: {},
      unreadConversations: new Set(),
    });
  });

  async function loaded() {
    const queryClient = new QueryClient();
    const unread = await queryClient.fetchQuery({
      queryKey: QUERY_KEYS.channelsUnread('ws-1'),
      queryFn: async () => counts,
    });
    const store = useWorkspaceStore.getState();
    store.hydrateUnreadChannels(unread.channel_ids);
    store.hydrateUnreadCounts(unread.counts);
    return queryClient;
  }

  it('puts back the loaded counts after a replay of messages they already held', async () => {
    const queryClient = await loaded();
    const { bumpChannelUnread } = useWorkspaceStore.getState();
    bumpChannelUnread('ch-1', false);
    bumpChannelUnread('ch-1', true);
    bumpChannelUnread('ch-1', false);
    expect(useWorkspaceStore.getState().unreadCounts['ch-1']).toBe(6);

    await recountAfterReplay(queryClient, 'ws-1');

    const s = useWorkspaceStore.getState();
    expect(s.unreadCounts['ch-1']).toBe(3);
    expect(s.mentionCounts['ch-1']).toBe(1);
  });

  it('leaves the counts alone for a replay in a workspace that is not open', async () => {
    const queryClient = await loaded();
    useWorkspaceStore.getState().bumpChannelUnread('ch-1', false);

    await recountAfterReplay(queryClient, 'ws-2');

    expect(useWorkspaceStore.getState().unreadCounts['ch-1']).toBe(4);
  });
});
