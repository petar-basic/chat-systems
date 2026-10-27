import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { EditProfileModal } from './EditProfileModal';

const saveProfile = vi.fn();
const setStatus = vi.fn();
const clearStatus = vi.fn();
let account: Record<string, string | null> = {};
let profile: Record<string, string | null> = {};

vi.mock('@/features/instance/hooks/useAuth', () => ({
  useCurrentUser: () => ({ data: { id: 'user-1', email: 'alice@dev.local' } }),
}));
vi.mock('@/features/profile/hooks/useProfile', () => ({
  useAccount: () => ({ data: account }),
  useWorkspaceProfile: () => ({ data: profile, isLoading: false }),
  useSaveWorkspaceProfile: () => ({
    profile: { mutateAsync: (...args: unknown[]) => saveProfile(...args) },
    setStatus: { mutateAsync: (...args: unknown[]) => setStatus(...args) },
    clearStatus: { mutateAsync: (...args: unknown[]) => clearStatus(...args) },
    refresh: vi.fn(),
  }),
}));
vi.mock('@/features/messaging', () => ({
  EmojiPicker: ({ onSelect, onClose }: { onSelect: (emoji: string) => void; onClose: () => void }) => (
    <button
      type="button"
      onClick={() => {
        onSelect(':shipit:');
        onClose();
      }}
    >
      pick shipit
    </button>
  ),
}));
vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) =>
    select({ currentWorkspace: { id: 'ws-1', name: 'Dev Team', instanceUrl: 'http://localhost:3000' } }),
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
    saveProfile.mockReset().mockResolvedValue(undefined);
    setStatus.mockReset().mockResolvedValue(undefined);
    clearStatus.mockReset().mockResolvedValue(undefined);
    account = { display_name: 'Alice', bio: 'Backend at Acme', avatar_url: null };
    profile = {
      workspace_id: 'ws-1',
      display_name: null,
      bio: null,
      avatar_url: null,
      status_emoji: null,
      status_text: null,
      status_expires_at: null,
    };
  });

  it('saves the profile and a new status with one button, then closes', async () => {
    const onClose = renderModal();

    fireEvent.change(screen.getByRole('textbox', { name: /Display name/ }), { target: { value: 'Alice J' } });
    fireEvent.click(screen.getByRole('button', { name: '🍕 At lunch' }));
    expect(screen.getByRole('combobox', { name: 'Clear after' })).toHaveValue('60');
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(saveProfile).toHaveBeenCalledWith({ display_name: 'Alice J', bio: '', avatar_url: '' });
    expect(setStatus).toHaveBeenCalledWith(expect.objectContaining({ emoji: '🍕', text: 'At lunch' }));
    expect(clearStatus).not.toHaveBeenCalled();
  });

  it("edits this workspace's profile and shows the account's where it has none", () => {
    renderModal();

    expect(screen.getByText('Dev Team')).toBeInTheDocument();
    const name = screen.getByRole('textbox', { name: /Display name/ });
    expect(name).toHaveValue('');
    expect(name).toHaveAttribute('placeholder', 'Alice');
    expect(screen.getByRole('textbox', { name: /Bio/ })).toHaveAttribute('placeholder', 'Backend at Acme');
  });

  it("goes back to the account's photo", async () => {
    account = { ...account, avatar_url: '/api/files/account.png' };
    profile = { ...profile, display_name: 'Ali', avatar_url: '/api/files/work.png' };
    const onClose = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Use account photo' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(saveProfile).toHaveBeenCalledWith({ display_name: 'Ali', bio: '', avatar_url: '' });
  });

  it('picks the status emoji from the picker, a workspace one included', async () => {
    renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Choose a status emoji' }));
    fireEvent.click(screen.getByRole('button', { name: 'pick shipit' }));
    expect(screen.getByRole('button', { name: 'Status emoji :shipit:' })).toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: 'Status' }), { target: { value: 'Shipping' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() =>
      expect(setStatus).toHaveBeenCalledWith(
        expect.objectContaining({ emoji: ':shipit:', text: 'Shipping' }),
      ),
    );
  });

  it('leaves an untouched status and its expiry alone', async () => {
    profile = {
      ...profile,
      status_emoji: '🍕',
      status_text: 'At lunch',
      status_expires_at: '2026-10-23T13:00:00Z',
    };
    const onClose = renderModal();

    expect(screen.getByTestId('status-expiry')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(setStatus).not.toHaveBeenCalled();
    expect(clearStatus).not.toHaveBeenCalled();
  });

  it('clears a status that was removed', async () => {
    profile = { ...profile, status_emoji: '🍕', status_text: 'At lunch' };
    const onClose = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Clear status' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(clearStatus).toHaveBeenCalled();
    expect(setStatus).not.toHaveBeenCalled();
  });

  it('stays open and says why when saving fails', async () => {
    saveProfile.mockRejectedValue(new Error('Display name is too long'));
    const onClose = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Save changes' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Display name is too long');
    expect(onClose).not.toHaveBeenCalled();
    expect(setStatus).not.toHaveBeenCalled();
  });
});
