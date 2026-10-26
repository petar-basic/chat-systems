import { useEffect, useRef } from 'react';
import { shortcutFor, stepSidebar, typingTarget, type Shortcut } from '@/features/navigation';

export type ShortcutHandlers = Partial<Record<Shortcut, () => void>>;

function stepTo(direction: 1 | -1, unreadOnly: boolean) {
  const target = stepSidebar(document, direction, unreadOnly);
  target?.scrollIntoView?.({ block: 'nearest' });
  target?.click();
}

const SIDEBAR_STEPS: Partial<Record<Shortcut, () => void>> = {
  previousChannel: () => stepTo(-1, false),
  nextChannel: () => stepTo(1, false),
  previousUnread: () => stepTo(-1, true),
  nextUnread: () => stepTo(1, true),
};

export function useWorkspaceShortcuts(handlers: ShortcutHandlers) {
  const latest = useRef(handlers);
  useEffect(() => {
    latest.current = handlers;
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.isComposing) return;
      const shortcut = shortcutFor(e, typingTarget(e.target));
      if (!shortcut) return;
      const global = shortcut === 'quickSwitcher' || shortcut === 'shortcuts';
      if (!global && document.querySelector('[role="dialog"]')) return;
      const run = latest.current[shortcut] ?? SIDEBAR_STEPS[shortcut];
      if (!run) return;
      e.preventDefault();
      run();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
}
