import { useCallback, useEffect, useMemo, useRef } from 'react';
import { instanceManager } from '@/lib/instances';
import { wsClient } from '@/lib/ws';
import { TYPING_TIMEOUT_MS, TYPING_REFRESH_MS } from '@/shared/constants';

export type TypingTarget = { channel_id: string } | { conversation_id: string };

export function typingTargetOf(
  channelId?: string | null,
  conversationId?: string | null,
): TypingTarget | null {
  if (channelId) return { channel_id: channelId };
  if (conversationId) return { conversation_id: conversationId };
  return null;
}

export function useTypingSignal(target: TypingTarget | null, instanceUrl?: string) {
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastStartRef = useRef(0);

  const send = useCallback(
    (frame: Record<string, unknown>) => {
      const ws = instanceUrl ? instanceManager.get(instanceUrl).ws : wsClient;
      ws.send(frame);
    },
    [instanceUrl],
  );

  const key = target ? JSON.stringify(target) : null;
  const stableTarget = useMemo(() => (key ? (JSON.parse(key) as TypingTarget) : null), [key]);

  const stopTyping = useCallback(() => {
    if (idleTimerRef.current) {
      clearTimeout(idleTimerRef.current);
      idleTimerRef.current = null;
    }
    if (!stableTarget || lastStartRef.current === 0) return;
    lastStartRef.current = 0;
    send({ type: 'typing.stop', ...stableTarget });
  }, [send, stableTarget]);

  const signalTyping = useCallback(() => {
    if (!stableTarget) return;
    const now = Date.now();
    if (now - lastStartRef.current > TYPING_REFRESH_MS) {
      lastStartRef.current = now;
      send({ type: 'typing.start', ...stableTarget });
    }
    if (idleTimerRef.current) clearTimeout(idleTimerRef.current);
    idleTimerRef.current = setTimeout(() => {
      idleTimerRef.current = null;
      lastStartRef.current = 0;
      send({ type: 'typing.stop', ...stableTarget });
    }, TYPING_TIMEOUT_MS);
  }, [send, stableTarget]);

  useEffect(() => stopTyping, [stopTyping]);

  return { signalTyping, stopTyping };
}
