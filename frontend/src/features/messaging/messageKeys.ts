import type { ShortcutKey } from '@/features/navigation';

export type MessageKeyAction =
  'previous' | 'next' | 'edit' | 'thread' | 'react' | 'later' | 'pin' | 'composer';

export const EDIT_MESSAGE_EVENT = 'chat:edit-message';

const LETTERS: Record<string, MessageKeyAction> = {
  e: 'edit',
  t: 'thread',
  r: 'react',
  a: 'later',
  p: 'pin',
};

export function messageKeyAction(e: ShortcutKey): MessageKeyAction | null {
  if (e.metaKey || e.ctrlKey || e.altKey) return null;
  if (e.key === 'ArrowUp') return 'previous';
  if (e.key === 'ArrowDown') return 'next';
  if (e.key === 'Enter') return 'thread';
  if (e.key === 'Escape') return 'composer';
  if (e.shiftKey) return null;
  return LETTERS[e.key.toLowerCase()] ?? null;
}

function paneOf(element: HTMLElement): ParentNode {
  return element.closest('[data-message-pane]') ?? document;
}

export function stepMessage(row: HTMLElement, direction: 1 | -1): HTMLElement | null {
  const rows = Array.from(paneOf(row).querySelectorAll<HTMLElement>('[data-message-row]'));
  return rows[rows.indexOf(row) + direction] ?? null;
}

export function moveFocus(target: HTMLElement | null): boolean {
  if (!target) return false;
  target.focus({ preventScroll: true });
  target.scrollIntoView?.({ block: 'nearest' });
  return true;
}

export function focusComposer(from: HTMLElement): boolean {
  const editors = paneOf(from).querySelectorAll<HTMLElement>('[data-composer] [contenteditable="true"]');
  const composer = editors[editors.length - 1];
  composer?.focus();
  return !!composer;
}

export function editLastOwnMessage(from: HTMLElement): boolean {
  const own = paneOf(from).querySelectorAll<HTMLElement>('[data-message-row][data-own="true"]');
  const last = own[own.length - 1];
  return last
    ? last.dispatchEvent(new CustomEvent(EDIT_MESSAGE_EVENT, { cancelable: true })) === false
    : false;
}
