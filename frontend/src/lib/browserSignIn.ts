import { invoke } from '@tauri-apps/api/core';
import { API_PATH } from './apiVersion';
import i18n from './i18n';

export type BrowserSignIn = 'sso' | 'passkey';

const SCHEME = 'chat-systems';
const TOKEN = /^[A-Za-z0-9_-]{16,128}$/;

function newNonce(): string {
  return Array.from(crypto.getRandomValues(new Uint8Array(24)), (b) => b.toString(16).padStart(2, '0')).join(
    '',
  );
}

export function browserSignInUrl(
  server: string,
  way: BrowserSignIn,
  nonce: string,
  invite?: string | null,
): string {
  if (way === 'passkey') {
    const url = new URL(`${server}/passkey/desktop`);
    url.searchParams.set('nonce', nonce);
    return url.toString();
  }
  const url = new URL(`${server}${API_PATH}/auth/oidc/start`);
  url.searchParams.set('client', 'desktop');
  url.searchParams.set('nonce', nonce);
  if (invite) url.searchParams.set('invite', invite);
  return url.toString();
}

export function codeFrom(callback: string, nonce: string): string | null {
  let url: URL;
  try {
    url = new URL(callback);
  } catch {
    return null;
  }
  const code = url.searchParams.get('code') ?? '';
  const ours = url.protocol === `${SCHEME}:` && url.host === 'sso' && url.searchParams.get('nonce') === nonce;
  return ours && TOKEN.test(code) ? code : null;
}

export async function signInInBrowser(
  server: string,
  way: BrowserSignIn,
  invite?: string | null,
): Promise<string> {
  const nonce = newNonce();
  let callback: string;
  try {
    callback = await invoke<string>('plugin:auth-session|start', {
      authUrl: browserSignInUrl(server, way, nonce, invite),
      callbackUrlScheme: SCHEME,
    });
  } catch (err) {
    if (err === 'user_cancelled') throw new DOMException('The sign-in sheet was closed', 'AbortError');
    throw new Error(i18n.t('auth:signIn.browserFailed'), { cause: err });
  }
  const code = codeFrom(callback, nonce);
  if (!code) throw new Error(i18n.t('auth:signIn.browserFailed'));
  return code;
}
