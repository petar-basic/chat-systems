import { beforeEach, describe, expect, it } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { applyReadState } from './readState';
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
});
