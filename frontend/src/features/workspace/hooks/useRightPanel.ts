import { useCallback, useState } from 'react';
import type { Message } from '@/stores/workspace';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

export type RightPanel =
  | { kind: 'members' }
  | { kind: 'settings' }
  | { kind: 'thread'; message: Message }
  | { kind: 'search' }
  | { kind: 'pins' }
  | { kind: 'channelMembers' }
  | { kind: 'integrations' }
  | { kind: 'customEmoji' }
  | { kind: 'userGroups' }
  | { kind: 'auditLog' }
  | { kind: 'scheduled' }
  | { kind: 'saved' }
  | { kind: 'reminders' }
  | { kind: 'slackImport' }
  | { kind: 'notifications' }
  | null;

export type PanelKind = NonNullable<RightPanel>['kind'];

/// The installed app's jump-list entries land here: `/app?panel=search` has to
/// open search, or the shortcut is a link that quietly does nothing.
function panelFromUrl(): RightPanel {
  if (typeof window === 'undefined') return null;
  const requested = new URLSearchParams(window.location.search).get('panel');
  if (!requested) return null;

  const opened = (['search', 'saved', 'reminders', 'scheduled', 'notifications'] as const).find(
    (kind) => kind === requested,
  );
  if (!opened) return null;

  const params = new URLSearchParams(window.location.search);
  params.delete('panel');
  const query = params.toString();
  window.history.replaceState({}, '', `${window.location.pathname}${query ? `?${query}` : ''}`);
  return { kind: opened };
}

export function useRightPanel(currentChannelId?: string, currentDmPartnerId?: string | null) {
  const [active, setActive] = useState<RightPanel>(panelFromUrl);

  const contextKey = `${currentChannelId ?? ''}:${currentDmPartnerId ?? ''}`;
  const [lastContextKey, setLastContextKey] = useState(contextKey);
  if (contextKey !== lastContextKey) {
    setLastContextKey(contextKey);
    setActive(null);
  }

  const toggle = useCallback((kind: Exclude<PanelKind, 'thread'>) => {
    setActive((p) => (p?.kind === kind ? null : { kind }));
  }, []);
  const openThread = useCallback((message: Message) => setActive({ kind: 'thread', message }), []);
  const close = useCallback(() => setActive(null), []);

  useEscapeToClose(close, !!active);

  return { active, toggle, openThread, close };
}
