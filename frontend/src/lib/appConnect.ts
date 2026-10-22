const KEY = 'app-connect-pending';

export interface PendingAppConnect {
  instanceUrl?: string;
  refreshMessageId?: string;
}

export const APP_NAMES: Record<string, string> = {
  jira: 'Jira',
  confluence: 'Confluence',
  github: 'GitHub',
  google_drive: 'Google Drive',
  figma: 'Figma',
};

export function appName(provider: string): string {
  return APP_NAMES[provider] ?? provider;
}

export function rememberPendingConnect(pending: PendingAppConnect) {
  try {
    sessionStorage.setItem(KEY, JSON.stringify(pending));
  } catch {
    return;
  }
}

export function takePendingConnect(): PendingAppConnect | null {
  try {
    const raw = sessionStorage.getItem(KEY);
    sessionStorage.removeItem(KEY);
    return raw ? (JSON.parse(raw) as PendingAppConnect) : null;
  } catch {
    return null;
  }
}
