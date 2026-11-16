import { useCallback, useEffect, useState } from 'react';
import type { AppRef, InstalledKind } from '@/features/workspace/hooks/useAppDirectory';
import { isSettingsSection, type SettingsSection } from './sections';
import { KINDS } from './apps/kinds';

export interface SettingsDialog {
  section: SettingsSection;
  inviting: boolean;
  app: AppRef | null;
}

function isKind(value: string): value is InstalledKind {
  return Object.hasOwn(KINDS, value);
}

function appFrom(value: string | null): AppRef | null {
  const [kind, id] = value?.split(':') ?? [];
  return kind && id && isKind(kind) ? { kind, id } : null;
}

function fromUrl(): SettingsDialog | null {
  if (typeof window === 'undefined') return null;
  const params = new URLSearchParams(window.location.search);
  const requested = params.get('settings');
  if (requested === null) return null;
  const app = appFrom(params.get('app'));
  return { section: isSettingsSection(requested) ? requested : 'general', inviting: false, app };
}

function forgetUrlRequest() {
  const params = new URLSearchParams(window.location.search);
  if (!params.has('settings')) return;
  params.delete('settings');
  params.delete('app');
  const query = params.toString();
  window.history.replaceState(
    window.history.state,
    '',
    `${window.location.pathname}${query ? `?${query}` : ''}${window.location.hash}`,
  );
}

export function useSettingsDialog() {
  const [dialog, setDialog] = useState<SettingsDialog | null>(fromUrl);
  useEffect(forgetUrlRequest, []);
  const open = useCallback(
    (section: SettingsSection, inviting = false, app: AppRef | null = null) =>
      setDialog({ section, inviting, app }),
    [],
  );
  const openApp = useCallback((app: AppRef) => setDialog({ section: 'apps', inviting: false, app }), []);
  const close = useCallback(() => setDialog(null), []);
  return { dialog, open, openApp, close };
}
