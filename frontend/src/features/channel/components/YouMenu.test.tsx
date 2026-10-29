import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { usePresenceStore } from '@/stores/presence';
import { useWorkspaceStore } from '@/stores/workspace';
import { YouMenu } from './YouMenu';

const put = vi.fn();

vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return {
    getApiForInstance: () => typedApiMock({ put: (...args) => put(...args) }),
    useCurrentApi: () => typedApiMock({}),
  };
});
vi.mock('@/features/notifications', () => ({ PauseMenuItems: () => null }));

const user = { id: 'user-1', display_name: 'Alice', email: 'alice@dev.local', avatar_url: null };

function renderMenu() {
  const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <YouMenu
        user={user}
        statusEmoji={null}
        statusText={null}
        unreadNotifCount={0}
        onOpenProfile={vi.fn()}
        onOpenPreferences={vi.fn()}
        onOpenScheduled={vi.fn()}
        onOpenLater={vi.fn()}
        onOpenNotifications={vi.fn()}
        onLogout={vi.fn()}
      />
    </QueryClientProvider>,
  );
  fireEvent.click(screen.getByTestId('open-you-menu'));
}

describe('YouMenu', () => {
  beforeEach(() => {
    put.mockReset();
    put.mockImplementation(async (_path: string, body: { away: boolean }) => body);
    usePresenceStore.setState({ statuses: { 'user-1': 'online' } });
    useWorkspaceStore.setState({ currentWorkspace: { id: 'ws-1', name: 'Acme' } as never });
  });

  it('sets you away and shows it at once', async () => {
    renderMenu();
    fireEvent.click(screen.getByRole('button', { name: 'Set yourself away' }));

    await waitFor(() => expect(put).toHaveBeenCalledWith('/users/me/presence', { away: true }));
    expect(usePresenceStore.getState().getStatus('user-1')).toBe('away');
    expect(screen.getByRole('img', { name: 'Status: Away' })).toBeInTheDocument();
  });

  it('offers to set you active while you are away', async () => {
    usePresenceStore.setState({ statuses: { 'user-1': 'away' } });
    renderMenu();
    fireEvent.click(screen.getByRole('button', { name: 'Set yourself active' }));

    await waitFor(() => expect(put).toHaveBeenCalledWith('/users/me/presence', { away: false }));
    expect(usePresenceStore.getState().getStatus('user-1')).toBe('online');
  });

  it('puts the dot back when the server refuses', async () => {
    put.mockRejectedValue(new Error('nope'));
    renderMenu();
    fireEvent.click(screen.getByRole('button', { name: 'Set yourself away' }));

    await waitFor(() => expect(usePresenceStore.getState().getStatus('user-1')).toBe('online'));
  });
});
