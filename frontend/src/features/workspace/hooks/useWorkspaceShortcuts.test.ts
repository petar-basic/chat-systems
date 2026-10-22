import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useWorkspaceShortcuts, type ShortcutHandlers } from './useWorkspaceShortcuts';

function press(target: EventTarget, init: KeyboardEventInit) {
  const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  target.dispatchEvent(event);
  return event;
}

function handlers(): Required<
  Pick<ShortcutHandlers, 'quickSwitcher' | 'shortcuts' | 'markChannelRead' | 'markAllRead'>
> {
  return { quickSwitcher: vi.fn(), shortcuts: vi.fn(), markChannelRead: vi.fn(), markAllRead: vi.fn() };
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('useWorkspaceShortcuts', () => {
  it('runs the bound handler and claims the key', () => {
    const h = handlers();
    renderHook(() => useWorkspaceShortcuts(h));
    expect(press(document.body, { key: 'k', metaKey: true }).defaultPrevented).toBe(true);
    press(document.body, { key: '/', ctrlKey: true });
    press(document.body, { key: 'Escape' });
    press(document.body, { key: 'Escape', shiftKey: true });
    expect(h.quickSwitcher).toHaveBeenCalledOnce();
    expect(h.shortcuts).toHaveBeenCalledOnce();
    expect(h.markChannelRead).toHaveBeenCalledOnce();
    expect(h.markAllRead).toHaveBeenCalledOnce();
  });

  it('does nothing while typing in a field', () => {
    const h = handlers();
    renderHook(() => useWorkspaceShortcuts(h));
    const input = document.createElement('input');
    document.body.append(input);
    const event = press(input, { key: 'Escape', shiftKey: true });
    expect(h.markAllRead).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);
  });

  it('leaves a key the editor already handled', () => {
    const h = handlers();
    renderHook(() => useWorkspaceShortcuts(h));
    const editor = document.createElement('div');
    editor.setAttribute('contenteditable', 'true');
    editor.addEventListener('keydown', (e) => e.preventDefault());
    document.body.append(editor);
    press(editor, { key: 'Escape' });
    expect(h.markChannelRead).not.toHaveBeenCalled();
  });

  it('keeps Esc for the dialog that is open', () => {
    const h = handlers();
    renderHook(() => useWorkspaceShortcuts(h));
    const dialog = document.createElement('div');
    dialog.setAttribute('role', 'dialog');
    document.body.append(dialog);
    press(document.body, { key: 'Escape' });
    press(document.body, { key: '/', metaKey: true });
    expect(h.markChannelRead).not.toHaveBeenCalled();
    expect(h.shortcuts).toHaveBeenCalledOnce();
  });

  it('opens the next channel and the next unread one from the sidebar', () => {
    renderHook(() => useWorkspaceShortcuts({}));
    const opened: string[] = [];
    for (const [name, active, unread] of [
      ['general', false, false],
      ['random', true, false],
      ['design', false, false],
      ['team', false, true],
    ] as const) {
      const row = document.createElement('button');
      row.dataset.navItem = 'true';
      row.dataset.navActive = String(active);
      row.dataset.navUnread = String(unread);
      row.addEventListener('click', () => opened.push(name));
      document.body.append(row);
    }
    press(document.body, { key: 'ArrowDown', altKey: true });
    press(document.body, { key: 'ArrowUp', altKey: true });
    press(document.body, { key: 'ArrowDown', altKey: true, shiftKey: true });
    expect(opened).toEqual(['design', 'general', 'team']);
  });
});
