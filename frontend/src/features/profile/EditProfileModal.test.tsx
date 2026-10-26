import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { EditProfileModal } from './EditProfileModal';

const patch = vi.fn();
const setStatus = vi.fn();
const clearStatus = vi.fn();
let me: Record<string, string | null> = {};

vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return {
    getApiForInstance: () => ({
      ...typedApiMock({ patch: (...args) => patch(...args) }),
      upload: vi.fn(),
    }),
  };
});
vi.mock('@/features/instance/hooks/useAuth', () => ({
  useCurrentUser: () => ({ data: { id: 'user-1', email: 'alice@dev.local' } }),
}));
vi.mock('@/features/profile/hooks/useStatus', () => ({
  useMyStatus: () => ({ data: me, isLoading: false }),
  useSetStatus: () => ({
    set: { mutateAsync: (...args: unknown[]) => setStatus(...args) },
    clear: { mutateAsync: (...args: unknown[]) => clearStatus(...args) },
  }),
}));
vi.mock('@/stores/instances', () => ({
  useInstanceStore: (select: (state: unknown) => unknown) =>
    select({ activeInstanceUrl: 'http://localhost:3000', updateInstanceUser: vi.fn() }),
}));
vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) => select({ currentWorkspace: { id: 'ws-1' } }),
}));

function renderModal() {
  const onClose = vi.fn();
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <EditProfileModal onClose={onClose} />
    </QueryClientProvider>,
  );
  return onClose;
}

describe('EditProfileModal', () => {
  beforeEach(() => {
    patch.mockReset().mockResolvedValue({ display_name: 'Alice J', avatar_url: null });
    setStatus.mockReset().mockResolvedValue(undefined);
    clearStatus.mockReset().mockResolvedValue(undefined);
    me = {
      display_name: 'Alice',
      bio: null,
      avatar_url: null,
      status_emoji: null,
      status_text: null,
      status_expires_at: null,
    };
  });

  it('saves the profile and a new status with one button, then closes', async () => {
    const onClose = renderModal();

    fireEvent.change(screen.getByRole('textbox', { name: 'Display name' }), { target: { value: 'Alice J' } });
    fireEvent.click(screen.getByRole('button', { name: '🍕 At lunch' }));
    expect(screen.getByRole('combobox', { name: 'Clear after' })).toHaveValue('60');
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(patch).toHaveBeenCalledWith('/users/me', { display_name: 'Alice J', bio: null, avatar_url: '' });
    expect(setStatus).toHaveBeenCalledWith(expect.objectContaining({ emoji: '🍕', text: 'At lunch' }));
    expect(clearStatus).not.toHaveBeenCalled();
  });

  it('leaves an untouched status and its expiry alone', async () => {
    me = { ...me, status_emoji: '🍕', status_text: 'At lunch', status_expires_at: '2026-10-23T13:00:00Z' };
    const onClose = renderModal();

    expect(screen.getByTestId('status-expiry')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(setStatus).not.toHaveBeenCalled();
    expect(clearStatus).not.toHaveBeenCalled();
  });

  it('clears a status that was removed', async () => {
    me = { ...me, status_emoji: '🍕', status_text: 'At lunch' };
    const onClose = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Clear status' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(clearStatus).toHaveBeenCalled();
    expect(setStatus).not.toHaveBeenCalled();
  });

  it('stays open and says why when saving fails', async () => {
    patch.mockRejectedValue(new Error('Display name is too long'));
    const onClose = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Display name is too long');
    expect(onClose).not.toHaveBeenCalled();
    expect(setStatus).not.toHaveBeenCalled();
  });
});
