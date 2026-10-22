import { describe, it, expect } from 'vitest';
import { shortcutFor, stepSidebar, typingTarget, type ShortcutKey } from './shortcuts';

function key(k: string, mods: Partial<ShortcutKey> = {}): ShortcutKey {
  return {
    key: k,
    code: k.length === 1 ? `Key${k.toUpperCase()}` : k,
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    ...mods,
  };
}

describe('shortcutFor', () => {
  it('maps every binding', () => {
    expect(shortcutFor(key('k', { metaKey: true }), null)).toBe('quickSwitcher');
    expect(shortcutFor(key('k', { ctrlKey: true }), null)).toBe('quickSwitcher');
    expect(shortcutFor(key('/', { metaKey: true }), null)).toBe('shortcuts');
    expect(shortcutFor(key('Å', { altKey: true, shiftKey: true, code: 'KeyA' }), null)).toBe('allUnreads');
    expect(shortcutFor(key('ArrowUp', { altKey: true }), null)).toBe('previousChannel');
    expect(shortcutFor(key('ArrowDown', { altKey: true }), null)).toBe('nextChannel');
    expect(shortcutFor(key('ArrowUp', { altKey: true, shiftKey: true }), null)).toBe('previousUnread');
    expect(shortcutFor(key('ArrowDown', { altKey: true, shiftKey: true }), null)).toBe('nextUnread');
    expect(shortcutFor(key('Escape'), null)).toBe('markChannelRead');
    expect(shortcutFor(key('Escape', { shiftKey: true }), null)).toBe('markAllRead');
  });

  it('leaves plain keys and the browser’s own chords alone', () => {
    expect(shortcutFor(key('ArrowDown'), null)).toBeNull();
    expect(shortcutFor(key('a'), null)).toBeNull();
    expect(shortcutFor(key('ArrowDown', { metaKey: true, altKey: true }), null)).toBeNull();
    expect(shortcutFor(key('k', { metaKey: true, shiftKey: true }), null)).toBeNull();
    expect(shortcutFor(key('Escape', { metaKey: true }), null)).toBeNull();
  });

  it('fires nothing but the ⌘ chords while typing in a field', () => {
    expect(shortcutFor(key('Escape'), 'field')).toBeNull();
    expect(shortcutFor(key('ArrowUp', { altKey: true }), 'field')).toBeNull();
    expect(shortcutFor(key('Å', { altKey: true, shiftKey: true, code: 'KeyA' }), 'field')).toBeNull();
    expect(shortcutFor(key('k', { metaKey: true }), 'field')).toBe('quickSwitcher');
  });

  it('lets the message box move between channels and mark read, as in Slack', () => {
    expect(shortcutFor(key('ArrowDown', { altKey: true }), 'editor')).toBe('nextChannel');
    expect(shortcutFor(key('Escape'), 'editor')).toBe('markChannelRead');
  });
});

describe('typingTarget', () => {
  it('tells fields from the rich editor', () => {
    const input = document.createElement('input');
    const editor = document.createElement('div');
    editor.setAttribute('contenteditable', 'true');
    document.body.append(input, editor);
    expect(typingTarget(input)).toBe('field');
    expect(typingTarget(document.createElement('textarea'))).toBe('field');
    expect(typingTarget(editor)).toBe('editor');
    expect(typingTarget(document.body)).toBeNull();
    expect(typingTarget(null)).toBeNull();
    input.remove();
    editor.remove();
  });
});

describe('stepSidebar', () => {
  function sidebar(rows: [string, { active?: boolean; unread?: boolean }][]) {
    const root = document.createElement('nav');
    for (const [name, { active = false, unread = false }] of rows) {
      const button = document.createElement('button');
      button.textContent = name;
      button.dataset.navItem = 'true';
      button.dataset.navActive = String(active);
      button.dataset.navUnread = String(unread);
      root.append(button);
    }
    return root;
  }

  const rows: [string, { active?: boolean; unread?: boolean }][] = [
    ['general', { unread: true }],
    ['design', {}],
    ['random', { active: true }],
    ['ana', {}],
    ['team', { unread: true }],
  ];

  it('steps to the neighbour in sidebar order', () => {
    const root = sidebar(rows);
    expect(stepSidebar(root, 1, false)?.textContent).toBe('ana');
    expect(stepSidebar(root, -1, false)?.textContent).toBe('design');
  });

  it('skips to the next unread one', () => {
    const root = sidebar(rows);
    expect(stepSidebar(root, 1, true)?.textContent).toBe('team');
    expect(stepSidebar(root, -1, true)?.textContent).toBe('general');
  });

  it('stops at the ends', () => {
    expect(stepSidebar(sidebar([['general', { active: true }]]), -1, false)).toBeNull();
    expect(stepSidebar(sidebar([['general', { active: true }]]), 1, false)).toBeNull();
  });

  it('starts from an end when nothing in the sidebar is open', () => {
    const root = sidebar([
      ['general', {}],
      ['design', {}],
    ]);
    expect(stepSidebar(root, 1, false)?.textContent).toBe('general');
    expect(stepSidebar(root, -1, false)?.textContent).toBe('design');
  });
});
