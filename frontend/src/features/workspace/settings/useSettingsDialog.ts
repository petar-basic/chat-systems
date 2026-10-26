import { useCallback, useState } from 'react';
import { isSettingsSection, type SettingsSection } from './sections';

export interface SettingsDialog {
  section: SettingsSection;
  inviting: boolean;
}

function fromUrl(): SettingsDialog | null {
  if (typeof window === 'undefined') return null;
  const params = new URLSearchParams(window.location.search);
  const requested = params.get('settings');
  if (requested === null) return null;
  params.delete('settings');
  const query = params.toString();
  window.history.replaceState({}, '', `${window.location.pathname}${query ? `?${query}` : ''}`);
  return { section: isSettingsSection(requested) ? requested : 'general', inviting: false };
}

export function useSettingsDialog() {
  const [dialog, setDialog] = useState<SettingsDialog | null>(fromUrl);
  const open = useCallback(
    (section: SettingsSection, inviting = false) => setDialog({ section, inviting }),
    [],
  );
  const close = useCallback(() => setDialog(null), []);
  return { dialog, open, close };
}
