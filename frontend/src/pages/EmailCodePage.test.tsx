import { describe, it, expect, vi, afterEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { ApiError } from '@/lib/errors';
import EmailCodePage from './EmailCodePage';

const typed = vi.hoisted(() => vi.fn());
const store = vi.hoisted(() => ({
  addInstanceWithEmailCode: vi.fn(),
  addInstanceWithPasskey: vi.fn(),
}));
const server = vi.hoisted(() => ({ emailCode: true }));

vi.mock('@/lib/api', () => ({
  api: { typed: (...args: unknown[]) => typed(...args) },
}));

vi.mock('@/stores/instances', () => ({
  useInstanceStore: () => ({
    addInstanceWithEmailCode: store.addInstanceWithEmailCode,
    addInstanceWithPasskey: store.addInstanceWithPasskey,
    loading: false,
    error: null,
    clearError: vi.fn(),
  }),
}));

vi.mock('@/features/instance/hooks/useInstanceInfo', () => ({
  useInstanceInfo: () => ({ data: { name: 'Northwind', email_code_sign_in: server.emailCode } }),
}));

function renderPage() {
  render(
    <MemoryRouter initialEntries={[{ pathname: '/sign-in/code', state: { email: 'ana@example.com' } }]}>
      <EmailCodePage />
    </MemoryRouter>,
  );
}

async function askForACode() {
  typed.mockResolvedValue({ status: 'sent' });
  renderPage();
  fireEvent.click(screen.getByRole('button', { name: 'Email me a code' }));
  await screen.findByRole('heading', { name: 'Check your email' });
  fireEvent.change(screen.getByLabelText('Sign-in code'), { target: { value: '123456' } });
}

describe('EmailCodePage', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    typed.mockReset();
    store.addInstanceWithEmailCode.mockReset();
    store.addInstanceWithPasskey.mockReset();
    server.emailCode = true;
  });

  it('emails a code to the address from the sign-in page and signs in with it', async () => {
    store.addInstanceWithEmailCode.mockResolvedValue(undefined);
    await askForACode();

    expect(screen.getByText('ana@example.com')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

    await waitFor(() =>
      expect(store.addInstanceWithEmailCode).toHaveBeenCalledWith(
        window.location.origin,
        'ana@example.com',
        '123456',
        undefined,
      ),
    );
  });

  it('keeps only six digits of what is typed or pasted', async () => {
    await askForACode();
    const input = screen.getByLabelText('Sign-in code');
    fireEvent.change(input, { target: { value: '12 34-5678' } });
    expect(input).toHaveValue('123456');
  });

  it('asks an account with two-factor for its code after the emailed one', async () => {
    store.addInstanceWithEmailCode.mockRejectedValueOnce(new ApiError(409, 'totp_required'));
    await askForACode();
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

    const totp = await screen.findByLabelText('Authentication code');
    store.addInstanceWithEmailCode.mockResolvedValue(undefined);
    fireEvent.change(totp, { target: { value: '654321' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

    await waitFor(() =>
      expect(store.addInstanceWithEmailCode).toHaveBeenLastCalledWith(
        window.location.origin,
        'ana@example.com',
        '123456',
        '654321',
      ),
    );
  });

  it('lets a passkey stand in for the two-factor code', async () => {
    vi.stubGlobal('PublicKeyCredential', function PublicKeyCredential() {});
    store.addInstanceWithEmailCode.mockRejectedValue(new ApiError(409, 'totp_or_passkey_required'));
    store.addInstanceWithPasskey.mockResolvedValue(undefined);
    await askForACode();
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

    fireEvent.click(await screen.findByRole('button', { name: 'Use a passkey instead' }));

    await waitFor(() =>
      expect(store.addInstanceWithPasskey).toHaveBeenCalledWith(window.location.origin, {
        email: 'ana@example.com',
        email_code: '123456',
      }),
    );
  });

  it('says so when the server does not send codes', () => {
    server.emailCode = false;
    renderPage();
    expect(screen.getByRole('heading', { name: 'Codes are off' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Email me a code' })).not.toBeInTheDocument();
  });
});
