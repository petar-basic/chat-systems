import type { MouseEvent } from 'react';
import { API_PATH } from '@/lib/apiVersion';
import { desktopShell } from '@/lib/desktopShell';

export function startSsoInDesktop(event: MouseEvent, invite?: string | null) {
  const startSso = desktopShell()?.startSso;
  if (!startSso) return;
  event.preventDefault();
  startSso(invite ?? null);
}

export function ssoStartUrl(invite?: string | null) {
  const start = `${window.location.origin}${API_PATH}/auth/oidc/start`;
  return invite ? `${start}?invite=${encodeURIComponent(invite)}` : start;
}
