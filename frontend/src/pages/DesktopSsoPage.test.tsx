import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import DesktopSsoPage from './DesktopSsoPage';

const { typed, openUrl, replacePage, requestPasskey } = vi.hoisted(() => ({
  typed: vi.fn(),
  openUrl: vi.fn(),
  replacePage: vi.fn(),
  requestPasskey: vi.fn(),
}));

vi.mock('@/lib/webauthn', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/webauthn')>()),
  requestPasskey,
}));

vi.mock('@/lib/instances', () => ({
  instanceManager: { get: () => ({ api: { typed } }) },
}));

vi.mock('@/lib/pageLocation', () => ({ openUrl, replacePage }));

const CODE = 'c29tZS1vbmUtdGltZS1jb2Rl';
const NONCE = '0123456789abcdef0123';

function renderAt(path: string, step: 'handoff' | 'complete' | 'passkey') {
  render(
    <MemoryRouter initialEntries={[path]}>
      <DesktopSsoPage step={step} />
    </MemoryRouter>,
  );
}

describe('DesktopSsoPage', () => {
  beforeEach(() => {
    typed.mockReset();
    openUrl.mockReset();
    replacePage.mockReset();
    requestPasskey.mockReset();
    vi.unstubAllGlobals();
  });

  it('hands the code from the browser back to the app', () => {
    renderAt(`/sso/desktop?code=${CODE}&nonce=${NONCE}`, 'handoff');

    const appUrl = `chat-systems://sso?code=${CODE}&nonce=${NONCE}`;
    expect(openUrl).toHaveBeenCalledWith(appUrl);
    expect(screen.getByTestId('sso-open-app')).toHaveAttribute('href', appUrl);
  });

  it('opens nothing for a link that was tampered with', () => {
    renderAt(`/sso/desktop?code=${CODE}&nonce=x%27%3B`, 'handoff');

    expect(openUrl).not.toHaveBeenCalled();
    expect(screen.getByText('This link is broken')).toBeInTheDocument();
  });

  it('trades the code for a session in the app and lands where the sign-in was going', async () => {
    typed.mockResolvedValue({ landing: '/app/w1' });
    renderAt(`/sso/complete?code=${CODE}`, 'complete');

    await waitFor(() => expect(replacePage).toHaveBeenCalledWith('/app/w1?sso=1'));
    expect(typed).toHaveBeenCalledOnce();
  });

  it('says so when the code was already used or expired', async () => {
    typed.mockRejectedValue(new Error('This sign-in expired, please try again'));
    renderAt(`/sso/complete?code=${CODE}`, 'complete');

    expect(await screen.findByText('This sign-in expired, please try again')).toBeInTheDocument();
    expect(replacePage).not.toHaveBeenCalled();
  });

  it('uses a passkey in the browser and hands the app a code, not a session', async () => {
    vi.stubGlobal('PublicKeyCredential', function PublicKeyCredential() {});
    typed
      .mockResolvedValueOnce({ challenge_id: 'challenge', options: { publicKey: {} } })
      .mockResolvedValueOnce({ code: CODE });
    requestPasskey.mockResolvedValue({ id: 'credential' });
    renderAt(`/passkey/desktop?nonce=${NONCE}`, 'passkey');

    fireEvent.click(screen.getByTestId('desktop-passkey'));

    await waitFor(() => expect(replacePage).toHaveBeenCalledWith(`/sso/desktop?code=${CODE}&nonce=${NONCE}`));
    expect(requestPasskey).toHaveBeenCalledWith({ publicKey: {} });
  });

  it('stays put when the person closes the passkey sheet', async () => {
    vi.stubGlobal('PublicKeyCredential', function PublicKeyCredential() {});
    typed.mockResolvedValueOnce({ challenge_id: 'challenge', options: { publicKey: {} } });
    requestPasskey.mockRejectedValue(new DOMException('closed', 'NotAllowedError'));
    renderAt(`/passkey/desktop?nonce=${NONCE}`, 'passkey');

    fireEvent.click(screen.getByTestId('desktop-passkey'));

    await waitFor(() => expect(screen.getByTestId('desktop-passkey')).not.toBeDisabled());
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(replacePage).not.toHaveBeenCalled();
  });

  it('refuses a passkey link the app did not make', () => {
    vi.stubGlobal('PublicKeyCredential', function PublicKeyCredential() {});
    renderAt('/passkey/desktop?nonce=x', 'passkey');

    expect(screen.getByText('This link is broken')).toBeInTheDocument();
    expect(screen.queryByTestId('desktop-passkey')).not.toBeInTheDocument();
  });
});
