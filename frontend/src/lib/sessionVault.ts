import { invoke } from '@tauri-apps/api/core';
import { inPhoneApp } from './phoneApp';

const PAGE_KEY = 'chat_tokens';

interface PageTokens {
  access: string;
  refresh: string;
}

function readPage(): Record<string, PageTokens> {
  try {
    const raw = localStorage.getItem(PAGE_KEY);
    return raw ? (JSON.parse(raw) as Record<string, PageTokens>) : {};
  } catch {
    return {};
  }
}

function writePage(server: string, tokens: PageTokens | null) {
  try {
    const map = readPage();
    if (tokens) map[server] = tokens;
    else delete map[server];
    localStorage.setItem(PAGE_KEY, JSON.stringify(map));
  } catch {
    return;
  }
}

const inDevice = new Map<string, string | null>();
let deviceWrites: Promise<void> = Promise.resolve();

function writeDevice(server: string, refresh: string | null): Promise<void> {
  if (inDevice.get(server) === refresh) return deviceWrites;
  inDevice.set(server, refresh);
  deviceWrites = deviceWrites
    .then(() =>
      refresh
        ? invoke<void>('save_session', { server, token: refresh })
        : invoke<void>('forget_session', { server }),
    )
    .catch(() => undefined);
  return deviceWrites;
}

export async function loadSession(server: string): Promise<string | null> {
  if (!inPhoneApp) return readPage()[server]?.refresh ?? null;
  const refresh = await invoke<string | null>('load_session', { server }).catch(() => null);
  inDevice.set(server, refresh);
  return refresh;
}

export function keepSession(server: string, access: string | null, refresh: string | null): Promise<void> {
  if (inPhoneApp) return writeDevice(server, refresh);
  writePage(server, access && refresh ? { access, refresh } : null);
  return Promise.resolve();
}

export function forgetSession(server: string): Promise<void> {
  return keepSession(server, null, null);
}
