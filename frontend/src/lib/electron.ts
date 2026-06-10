export interface ElectronRefreshResult {
  access_token: string;
  user: unknown;
  expires_in: number;
}

interface ElectronAuth {
  setRefresh: (url: string, token: string) => Promise<void>;
  clearRefresh: (url: string) => Promise<void>;
  refresh: (url: string) => Promise<ElectronRefreshResult | null>;
}

interface ElectronAPI {
  platform: string;
  isElectron: boolean;
  send: (channel: string, data?: unknown) => void;
  on: (channel: string, func: (...args: unknown[]) => void) => void;
  auth: ElectronAuth;
}

declare global {
  interface Window {
    electronAPI?: ElectronAPI;
  }
}

const electron = typeof window !== 'undefined' ? window.electronAPI : undefined;

export const isElectron = !!electron?.isElectron;
export const electronAuth = electron?.auth ?? null;

export function setBadgeCount(count: number): void {
  electron?.send('badge-count', count);
}

export function showNativeNotification(title: string, body: string): void {
  electron?.send('notify', { title, body });
}

export function onDeepLink(handler: (url: string) => void): void {
  electron?.on('deep-link', (...args) => {
    const url = args[0];
    if (typeof url === 'string') handler(url);
  });
}
