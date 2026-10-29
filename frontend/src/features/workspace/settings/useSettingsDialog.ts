import { useCallback, useState } from 'react';
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
  params.delete('settings');
  params.delete('app');
  const query = params.toString();
  window.history.replaceState({}, '', `${window.location.pathname}${query ? `?${query}` : ''}`);
  return { section: isSettingsSection(requested) ? requested : 'general', inviting: false, app };
}

export function useSettingsDialog() {
  const [dialog, setDialog] = useState<SettingsDialog | null>(fromUrl);
  const open = useCallback(
    (section: SettingsSection, inviting = false, app: AppRef | null = null) =>
      setDialog({ section, inviting, app }),
    [],
  );
  const openApp = useCallback((app: AppRef) => setDialog({ section: 'apps', inviting: false, app }), []);
  const close = useCallback(() => setDialog(null), []);
  return { dialog, open, openApp, close };
}
