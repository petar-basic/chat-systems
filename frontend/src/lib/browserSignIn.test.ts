import { afterEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { browserSignInUrl, codeFrom, signInInBrowser } from './browserSignIn';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const NONCE = 'n'.repeat(48);
const CODE = 'c'.repeat(43);

function nonceSent(): string {
  const [, args] = vi.mocked(invoke).mock.calls[0] as [string, { authUrl: string }];
  return new URL(args.authUrl).searchParams.get('nonce') ?? '';
}

describe('signing in to a server from the phone app', () => {
  afterEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it('starts single sign-on the way the desktop app does, with the invite it was given', () => {
    const url = new URL(browserSignInUrl('https://chat.example', 'sso', NONCE, 'inv-1'));
    expect(url.origin + url.pathname).toBe('https://chat.example/api/v1/auth/oidc/start');
    expect(Object.fromEntries(url.searchParams)).toEqual({
      client: 'desktop',
      nonce: NONCE,
      invite: 'inv-1',
    });
  });

  it('opens the server passkey page for a passkey', () => {
    expect(browserSignInUrl('https://chat.example', 'passkey', NONCE)).toBe(
      `https://chat.example/passkey/desktop?nonce=${NONCE}`,
    );
  });

  it('takes the code only from a hand-back for this sign-in', () => {
    expect(codeFrom(`chat-systems://sso?code=${CODE}&nonce=${NONCE}`, NONCE)).toBe(CODE);
    expect(codeFrom(`chat-systems://sso?code=${CODE}&nonce=${'x'.repeat(48)}`, NONCE)).toBeNull();
    expect(codeFrom(`chat-systems://open?code=${CODE}&nonce=${NONCE}`, NONCE)).toBeNull();
    expect(codeFrom(`chat-systems://sso?code=short&nonce=${NONCE}`, NONCE)).toBeNull();
    expect(codeFrom('garbage', NONCE)).toBeNull();
  });

  it('returns the code the server handed back', async () => {
    vi.mocked(invoke).mockImplementation(async () => `chat-systems://sso?code=${CODE}&nonce=${nonceSent()}`);
    await expect(signInInBrowser('https://chat.example', 'sso')).resolves.toBe(CODE);
    expect(vi.mocked(invoke).mock.calls[0]?.[0]).toBe('plugin:auth-session|start');
  });

  it('treats closing the sheet as a cancel, not a failure', async () => {
    vi.mocked(invoke).mockRejectedValue('user_cancelled');
    await expect(signInInBrowser('https://chat.example', 'passkey')).rejects.toMatchObject({
      name: 'AbortError',
    });
  });

  it('fails when the sheet breaks or hands back something else', async () => {
    vi.mocked(invoke).mockRejectedValue('no browser');
    await expect(signInInBrowser('https://chat.example', 'sso')).rejects.toThrow(Error);
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(`chat-systems://sso?code=${CODE}&nonce=${NONCE}`);
    await expect(signInInBrowser('https://chat.example', 'sso')).rejects.not.toMatchObject({
      name: 'AbortError',
    });
  });
});
