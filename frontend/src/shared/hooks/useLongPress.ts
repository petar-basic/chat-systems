import { useCallback, useRef, type PointerEvent as ReactPointerEvent } from 'react';

const HOLD_MS = 400;
const MOVE_TOLERANCE_PX = 12;
const INTERACTIVE = 'button, a, input, textarea, [contenteditable="true"]';

/**
 * A finger cannot hover, so on a touch device holding a message is what opens
 * its actions. A mouse is left alone: it has the hover toolbar.
 */
export function useLongPress(onLongPress: () => void, enabled = true) {
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const origin = useRef<{ x: number; y: number } | null>(null);
  const fromTouch = useRef(false);

  const cancel = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    origin.current = null;
  }, []);

  const onPointerDown = useCallback(
    (e: ReactPointerEvent<HTMLElement>) => {
      fromTouch.current = e.pointerType !== 'mouse';
      if (!enabled || !fromTouch.current) return;
      if ((e.target as HTMLElement).closest(INTERACTIVE)) return;
      origin.current = { x: e.clientX, y: e.clientY };
      timer.current = setTimeout(() => {
        timer.current = null;
        navigator.vibrate?.(8);
        onLongPress();
      }, HOLD_MS);
    },
    [enabled, onLongPress],
  );

  const onPointerMove = useCallback(
    (e: ReactPointerEvent<HTMLElement>) => {
      const start = origin.current;
      if (!start || !timer.current) return;
      const moved =
        Math.abs(e.clientX - start.x) > MOVE_TOLERANCE_PX ||
        Math.abs(e.clientY - start.y) > MOVE_TOLERANCE_PX;
      if (moved) cancel();
    },
    [cancel],
  );

  return {
    onPointerDown,
    onPointerMove,
    onPointerUp: cancel,
    onPointerCancel: cancel,
    onPointerLeave: cancel,
    onContextMenu: (e: React.MouseEvent) => {
      if (enabled && fromTouch.current) e.preventDefault();
    },
  };
}
