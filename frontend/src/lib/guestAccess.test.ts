import { afterEach, describe, expect, it } from 'vitest';
import { QueryClient } from '@tanstack/react-query';
import { useToastStore } from '@/shared/components/Toast';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore, type Workspace } from '@/stores/workspace';
import { sayGuestAccessEnded } from './guestAccess';

describe('sayGuestAccessEnded', () => {
  afterEach(() => {
    useToastStore.setState({ toasts: [] });
    useWorkspaceStore.setState({ currentWorkspace: null });
  });

  it('names the workspace and stays until it is dismissed', () => {
    const queryClient = new QueryClient();
    queryClient.setQueryData<Workspace[]>(QUERY_KEYS.workspaces(), [
      { id: 'w1', name: 'Acme Vendors' } as Workspace,
    ]);
    sayGuestAccessEnded(queryClient, 'w1');
    const [shown] = useToastStore.getState().toasts;
    expect(shown.message).toBe('Your guest access to Acme Vendors has ended.');
    expect(shown.durationMs).toBe(0);
  });

  it('still says why when the workspace is no longer known', () => {
    sayGuestAccessEnded(new QueryClient(), 'unknown');
    expect(useToastStore.getState().toasts[0].message).toBe('Your guest access to a workspace has ended.');
  });
});
