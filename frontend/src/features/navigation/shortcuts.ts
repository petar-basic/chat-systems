export type Shortcut =
  | 'quickSwitcher'
  | 'shortcuts'
  | 'allUnreads'
  | 'previousChannel'
  | 'nextChannel'
  | 'previousUnread'
  | 'nextUnread'
  | 'markChannelRead'
  | 'markAllRead';

export type TypingTarget = 'field' | 'editor' | null;

export interface ShortcutKey {
  key: string;
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

export function typingTarget(target: EventTarget | null): TypingTarget {
  if (!(target instanceof HTMLElement)) return null;
  if (target.matches('input, textarea, select')) return 'field';
  if (target.isContentEditable || target.closest('[contenteditable="true"]')) return 'editor';
  return null;
}

export function shortcutFor(e: ShortcutKey, typing: TypingTarget): Shortcut | null {
  const mod = e.metaKey || e.ctrlKey;
  if (mod && !e.altKey) {
    if (e.key.toLowerCase() === 'k' && !e.shiftKey) return 'quickSwitcher';
    if (e.key === '/') return 'shortcuts';
    return null;
  }
  if (mod || typing === 'field') return null;
  if (e.altKey && e.shiftKey && e.code === 'KeyA') return 'allUnreads';
  if (e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
    const up = e.key === 'ArrowUp';
    if (e.shiftKey) return up ? 'previousUnread' : 'nextUnread';
    return up ? 'previousChannel' : 'nextChannel';
  }
  if (e.key === 'Escape' && !e.altKey) return e.shiftKey ? 'markAllRead' : 'markChannelRead';
  return null;
}

export function stepSidebar(root: ParentNode, direction: 1 | -1, unreadOnly: boolean): HTMLElement | null {
  const items = Array.from(root.querySelectorAll<HTMLElement>('[data-nav-item]'));
  const current = items.findIndex((item) => item.dataset.navActive === 'true');
  const start = current === -1 ? (direction === 1 ? -1 : items.length) : current;
  for (let i = start + direction; i >= 0 && i < items.length; i += direction) {
    if (!unreadOnly || items[i].dataset.navUnread === 'true') return items[i];
  }
  return null;
}

export function isApplePlatform(): boolean {
  return /Mac|iPhone|iPad|iPod/.test(navigator.platform || navigator.userAgent);
}
