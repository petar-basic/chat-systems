import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import i18n from '@/lib/i18n';
import PasskeysPanel from './PasskeysPanel';

const get = vi.fn();
const post = vi.fn();
const patch = vi.fn();
const createPasskey = vi.fn();

vi.mock('@/lib/api', async () => {
  const { typedApiMock } = await import('@/test/typedApiMock');
  return {
    api: typedApiMock({
      get: (...args) => get(...args),
      post: (...args) => post(...args),
      patch: (...args) => patch(...args),
    }),
  };
});

vi.mock('@/lib/webauthn', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/webauthn')>()),
  createPasskey: (...args: unknown[]) => createPasskey(...args),
}));

vi.mock('@/shared/components/Toast', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

const laptop = {
  id: 'a1b2c3d4-0000-4000-8000-000000000001',
  name: 'Laptop',
  created_at: '2026-11-01T10:00:00Z',
  last_used_at: null,
};

function renderPanel() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <PasskeysPanel />
    </QueryClientProvider>,
  );
  return client;
}

describe('PasskeysPanel', () => {
  beforeEach(() => {
    get.mockReset();
    post.mockReset();
    patch.mockReset();
    createPasskey.mockReset();
    vi.stubGlobal('PublicKeyCredential', function PublicKeyCredential() {});
  });

  afterEach(async () => {
    vi.unstubAllGlobals();
    await i18n.changeLanguage('en');
  });

  it('in Japanese names a passkey as text, whatever it is called', async () => {
    const name = '<img src=x onerror="alert(1)"> {{name}}';
    get.mockResolvedValue({ offered: true, data: [{ ...laptop, name }] });
    await i18n.changeLanguage('ja');
    renderPanel();

    expect(await screen.findByText(name)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `${name} の名前を変更` })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: `${name} を削除` }));

    expect(screen.getByTestId('passkey-confirm')).toHaveTextContent(
      `「${name}」を削除します。パスワードか認証アプリのコードで確認してください。`,
    );
    expect(screen.getByText(/に追加 · 未使用/)).toBeInTheDocument();
    expect(document.querySelector('img')).toBeNull();
  });

  it('lists the passkeys with when each was last used', async () => {
    get.mockResolvedValue({ offered: true, data: [laptop] });
    renderPanel();

    expect(await screen.findByText('Laptop')).toBeInTheDocument();
    expect(screen.getByText(/never used/)).toBeInTheDocument();
    expect(get).toHaveBeenCalledWith('/auth/passkeys');
  });

  it('asks for the password before the browser makes a passkey', async () => {
    get.mockResolvedValue({ offered: true, data: [] });
    post.mockImplementation(async (path: string) =>
      path === '/auth/passkeys/registration'
        ? { challenge_id: 'challenge', options: { publicKey: {} } }
        : { ...laptop, name: 'Work laptop' },
    );
    createPasskey.mockResolvedValue({ id: 'credential' });
    renderPanel();

    fireEvent.click(await screen.findByTestId('passkey-add'));
    expect(screen.getByTestId('passkey-confirm-submit')).toBeDisabled();
    expect(createPasskey).not.toHaveBeenCalled();

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Work laptop' } });
    fireEvent.change(screen.getByLabelText('Password or code'), { target: { value: 'correct horse' } });
    fireEvent.click(screen.getByTestId('passkey-confirm-submit'));

    await waitFor(() =>
      expect(post).toHaveBeenCalledWith('/auth/passkeys', {
        challenge_id: 'challenge',
        name: 'Work laptop',
        credential: { id: 'credential' },
      }),
    );
    expect(post).toHaveBeenCalledWith('/auth/passkeys/registration', { password: 'correct horse' });
    expect(createPasskey).toHaveBeenCalledWith({ publicKey: {} });
  });

  it('removes one after a code from the authenticator app', async () => {
    get.mockResolvedValue({ offered: true, data: [laptop] });
    post.mockResolvedValue({ status: 'removed' });
    renderPanel();

    fireEvent.click(await screen.findByTestId('passkey-remove'));
    fireEvent.change(screen.getByLabelText('Password or code'), { target: { value: '123456' } });
    fireEvent.click(screen.getByTestId('passkey-confirm-submit'));

    await waitFor(() => expect(post).toHaveBeenCalledWith('/auth/passkeys/{id}/remove', { code: '123456' }));
  });

  it('has the two-factor status fetched again once a passkey is gone', async () => {
    get.mockResolvedValue({ offered: true, data: [laptop] });
    post.mockResolvedValue({ status: 'removed' });
    const client = renderPanel();
    const invalidate = vi.spyOn(client, 'invalidateQueries');

    fireEvent.click(await screen.findByTestId('passkey-remove'));
    fireEvent.change(screen.getByLabelText('Password or code'), { target: { value: '123456' } });
    fireEvent.click(screen.getByTestId('passkey-confirm-submit'));

    await waitFor(() => expect(invalidate).toHaveBeenCalledWith({ queryKey: ['totp-status'] }));
  });

  it('renames one in place', async () => {
    get.mockResolvedValue({ offered: true, data: [laptop] });
    patch.mockResolvedValue({ status: 'renamed' });
    renderPanel();

    fireEvent.click(await screen.findByRole('button', { name: 'Rename Laptop' }));
    fireEvent.change(screen.getByTestId('passkey-rename-input'), { target: { value: 'Old laptop' } });
    fireEvent.click(screen.getByTestId('passkey-rename-save'));

    await waitFor(() => expect(patch).toHaveBeenCalledWith('/auth/passkeys/{id}', { name: 'Old laptop' }));
  });

  it('offers nothing to add for an account that signs in through SSO', async () => {
    get.mockResolvedValue({ offered: false, data: [] });
    renderPanel();

    expect(await screen.findByText(/single sign-on/)).toBeInTheDocument();
    expect(screen.queryByTestId('passkey-add')).not.toBeInTheDocument();
  });
});
