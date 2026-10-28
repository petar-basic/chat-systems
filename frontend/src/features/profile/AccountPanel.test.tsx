import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import AccountPanel from './AccountPanel';

const patch = vi.fn();
const upload = vi.fn();
const updateInstanceUser = vi.fn();

vi.mock('@/shared/hooks/useCurrentApi', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return {
    getApiForInstance: () => ({
      ...typedApiMock({ patch: (...args) => patch(...args) }),
      upload: (...args: unknown[]) => upload(...args),
    }),
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
    upload.mockReset();
  });

  it('puts a new photo on the account itself, not in the open workspace', async () => {
    upload.mockResolvedValue({
      display_name: 'Alice',
      avatar_url: 'http://localhost:3000/api/files/download/avatars/user-1/a.png',
    });
    renderPanel();

    const file = new File(['png'], 'me.png', { type: 'image/png' });
    fireEvent.change(screen.getByLabelText('Profile photo file'), { target: { files: [file] } });

    await waitFor(() =>
      expect(updateInstanceUser).toHaveBeenCalledWith(
        'http://localhost:3000',
        expect.objectContaining({
          avatar_url: 'http://localhost:3000/api/files/download/avatars/user-1/a.png',
        }),
      ),
    );
    const [path, form] = upload.mock.calls[0] as [string, FormData];
    expect(path).toBe('/users/me/avatar');
    expect(form.get('file')).toBe(file);
    expect(patch).not.toHaveBeenCalled();
  });

  it('says why a photo was refused', async () => {
    upload.mockRejectedValue(new Error('A photo is at most 10 MB'));
    renderPanel();

    fireEvent.change(screen.getByLabelText('Profile photo file'), {
      target: { files: [new File(['x'], 'big.png', { type: 'image/png' })] },
    });

    expect(await screen.findByRole('alert')).toHaveTextContent('A photo is at most 10 MB');
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
