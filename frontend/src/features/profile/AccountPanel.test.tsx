import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import AccountPanel from './AccountPanel';

const patch = vi.fn();
const updateInstanceUser = vi.fn();

vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return {
    getApiForInstance: () => ({ ...typedApiMock({ patch: (...args) => patch(...args) }), upload: vi.fn() }),
  };
});
vi.mock('@/features/instance/hooks/useAuth', () => ({
  useCurrentUser: () => ({ data: { id: 'user-1', email: 'alice@dev.local', display_name: 'Alice' } }),
}));
vi.mock('@/features/profile/hooks/useProfile', () => ({
  useAccount: () => ({ data: { display_name: 'Alice', bio: null, avatar_url: null } }),
}));
vi.mock('@/stores/instances', () => ({
  useInstanceStore: (select: (state: unknown) => unknown) =>
    select({ activeInstanceUrl: 'http://localhost:3000', updateInstanceUser }),
}));
vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) => select({ currentWorkspace: { id: 'ws-1' } }),
}));

function renderPanel() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <AccountPanel />
    </QueryClientProvider>,
  );
}

describe('AccountPanel', () => {
  beforeEach(() => {
    patch.mockReset().mockResolvedValue({ display_name: 'Alice Jones', avatar_url: null, bio: 'Hi' });
    updateInstanceUser.mockReset();
  });

  it("saves the account's name and bio and updates the signed-in user", async () => {
    renderPanel();

    fireEvent.change(screen.getByRole('textbox', { name: 'Display name' }), {
      target: { value: 'Alice Jones' },
    });
    fireEvent.change(screen.getByRole('textbox', { name: /Bio/ }), { target: { value: ' Hi ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save account' }));

    await waitFor(() =>
      expect(updateInstanceUser).toHaveBeenCalledWith(
        'http://localhost:3000',
        expect.objectContaining({ display_name: 'Alice Jones' }),
      ),
    );
    expect(patch).toHaveBeenCalledWith('/users/me', {
      display_name: 'Alice Jones',
      bio: 'Hi',
      avatar_url: '',
    });
  });

  it('needs a name, since every workspace without its own falls back to it', () => {
    renderPanel();

    fireEvent.change(screen.getByRole('textbox', { name: 'Display name' }), { target: { value: ' ' } });
    fireEvent.submit(screen.getByTestId('account-panel'));

    expect(screen.getByRole('alert')).toHaveTextContent('Enter a display name.');
    expect(patch).not.toHaveBeenCalled();
  });
});
