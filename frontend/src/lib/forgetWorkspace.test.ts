import { describe, it, expect, beforeEach } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore, type Channel, type Workspace } from '@/stores/workspace';
import { forgetWorkspace } from './forgetWorkspace';

const northwind = { id: 'w1', name: 'Northwind' } as Workspace;
const lab = { id: 'w2', name: 'Lab' } as Workspace;
const general = { id: 'c1', name: 'general' } as Channel;

describe('forgetWorkspace', () => {
  let queryClient: QueryClient;

  beforeEach(() => {
    queryClient = new QueryClient();
    queryClient.setQueryData(QUERY_KEYS.workspacesList('https://chat.example.com'), [northwind, lab]);
    queryClient.setQueryData(QUERY_KEYS.workspace('w1'), northwind);
    queryClient.setQueryData(QUERY_KEYS.workspaceChannels('w1'), [general]);
    queryClient.setQueryData(QUERY_KEYS.workspaceMembers('w1'), [{ user_id: 'me' }]);
    queryClient.setQueryData(QUERY_KEYS.workspaceChannels('w2'), [general]);
    useWorkspaceStore.setState({ currentWorkspace: northwind, currentChannel: general });
  });

  it('drops the workspace from the lists, its channels and members, and what is open', () => {
    forgetWorkspace(queryClient, 'w1');

    expect(queryClient.getQueryData(QUERY_KEYS.workspacesList('https://chat.example.com'))).toEqual([lab]);
    expect(queryClient.getQueryData(QUERY_KEYS.workspace('w1'))).toEqual(northwind);
    expect(queryClient.getQueryData(QUERY_KEYS.workspaceChannels('w1'))).toBeUndefined();
    expect(queryClient.getQueryData(QUERY_KEYS.workspaceMembers('w1'))).toBeUndefined();
    expect(queryClient.getQueryData(QUERY_KEYS.workspaceChannels('w2'))).toEqual([general]);
    expect(useWorkspaceStore.getState().currentWorkspace).toBeNull();
    expect(useWorkspaceStore.getState().currentChannel).toBeNull();
  });

  it('leaves another open workspace alone', () => {
    useWorkspaceStore.setState({ currentWorkspace: lab });

    forgetWorkspace(queryClient, 'w1');

    expect(useWorkspaceStore.getState().currentWorkspace).toBe(lab);
    expect(useWorkspaceStore.getState().currentChannel).toBe(general);
  });
});
