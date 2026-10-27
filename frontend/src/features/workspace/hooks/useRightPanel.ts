import { useCallback, useState } from 'react';
import type { Message } from '@/stores/workspace';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';

export type RightPanel =
  | { kind: 'thread'; message: Message; replyId?: string }
  | { kind: 'search' }
  | { kind: 'pins' }
  | { kind: 'files' }
  | { kind: 'channelMembers'; adding?: boolean }
  | { kind: 'workflows'; workflowId?: string }
  | { kind: 'scheduled' }
  | { kind: 'later' }
  | { kind: 'notifications' }
  | null;

export type PanelKind = NonNullable<RightPanel>['kind'];

/// The installed app's jump-list entries land here: `/app?panel=search` has to
/// open search, or the shortcut is a link that quietly does nothing.
function panelFromUrl(): RightPanel {
  if (typeof window === 'undefined') return null;
  const requested = new URLSearchParams(window.location.search).get('panel');
  if (!requested) return null;

  const opened = (['search', 'later', 'scheduled', 'notifications'] as const).find(
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
  const addChannelMembers = useCallback(() => setActive({ kind: 'channelMembers', adding: true }), []);
  const openWorkflow = useCallback((workflowId: string) => setActive({ kind: 'workflows', workflowId }), []);
  const openThread = useCallback(
    (message: Message, replyId?: string) => setActive({ kind: 'thread', message, replyId }),
    [],
  );
  const close = useCallback(() => setActive(null), []);

  useEscapeToClose(close, !!active);

  return { active, toggle, openThread, addChannelMembers, openWorkflow, close };
}
