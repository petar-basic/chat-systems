import { describe, it, expect, beforeEach } from 'vitest';
import { useWorkspaceStore, type Channel, type Workspace } from './workspace';

const workspace = (id: string, name: string) => ({ id, name }) as Workspace;
const channel = { id: 'ch-1', name: 'general' } as Channel;

describe('refreshWorkspace', () => {
  beforeEach(() => {
    useWorkspaceStore.setState({ currentWorkspace: workspace('ws-1', 'Old'), currentChannel: channel });
  });

  it('replaces the current workspace and keeps the channel', () => {
    useWorkspaceStore.getState().refreshWorkspace(workspace('ws-1', 'New'));

    expect(useWorkspaceStore.getState().currentWorkspace?.name).toBe('New');
    expect(useWorkspaceStore.getState().currentChannel).toBe(channel);
  });

  it('ignores a workspace that is not the current one', () => {
    useWorkspaceStore.getState().refreshWorkspace(workspace('ws-2', 'Other'));

    expect(useWorkspaceStore.getState().currentWorkspace?.name).toBe('Old');
  });
});
