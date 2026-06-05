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
  | { kind: 'notifications' }
  | null;

export type PanelKind = NonNullable<RightPanel>['kind'];

export function useRightPanel(currentChannelId?: string, currentDmPartnerId?: string | null) {
  const [active, setActive] = useState<RightPanel>(null);

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
