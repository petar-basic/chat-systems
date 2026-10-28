import { describe, it, expect, afterEach } from 'vitest';
import { queryClient } from './queryClient';
import { applyChannelArchived } from './channelArchive';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore, type Channel } from '@/stores/workspace';

const channel = { id: 'c1', workspace_id: 'w1', name: 'launch', is_archived: false } as Channel;

describe('applyChannelArchived', () => {
  afterEach(() => {
    queryClient.clear();
    useWorkspaceStore.setState({ currentChannel: null });
  });

  it('turns the open channel read-only and refreshes the lists it appears in', () => {
    useWorkspaceStore.setState({ currentChannel: channel });
    queryClient.setQueryData(QUERY_KEYS.channel('c1'), channel);
    queryClient.setQueryData(QUERY_KEYS.workspaceChannels('w1'), [channel]);
    queryClient.setQueryData(QUERY_KEYS.channelsBrowse('w1'), [channel]);

    applyChannelArchived(queryClient, 'c1', 'w1', true);

    expect(useWorkspaceStore.getState().currentChannel?.is_archived).toBe(true);
    expect(queryClient.getQueryData<Channel>(QUERY_KEYS.channel('c1'))?.is_archived).toBe(true);
    expect(queryClient.getQueryState(QUERY_KEYS.workspaceChannels('w1'))?.isInvalidated).toBe(true);
    expect(queryClient.getQueryState(QUERY_KEYS.channelsBrowse('w1'))?.isInvalidated).toBe(true);

    applyChannelArchived(queryClient, 'c1', 'w1', false);
    expect(useWorkspaceStore.getState().currentChannel?.is_archived).toBe(false);
  });

  it('leaves another open channel alone', () => {
    const other = { ...channel, id: 'c2' };
    useWorkspaceStore.setState({ currentChannel: other });

    applyChannelArchived(queryClient, 'c1', 'w1', true);

    expect(useWorkspaceStore.getState().currentChannel).toBe(other);
  });
});
