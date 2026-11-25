import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useDisplayStore } from '@/stores/display';
import { KeyboardShortcutsDialog } from './KeyboardShortcutsDialog';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const open = () => render(<KeyboardShortcutsDialog onClose={vi.fn()} />);

const headings = () => screen.getAllByRole('heading', { level: 3 }).map((h) => h.textContent);

const table = () =>
  Object.fromEntries(
    Array.from(document.querySelectorAll('dl > div')).map((row) => [
      row.querySelector('dt')?.textContent,
      row.querySelector('dd')?.textContent,
    ]),
  );

const onPlatform = (platform: string) => vi.spyOn(navigator, 'platform', 'get').mockReturnValue(platform);

const flatten = (value: unknown, prefix: string): string[] =>
  value && typeof value === 'object'
    ? Object.entries(value).flatMap(([key, inner]) => flatten(inner, `${prefix}.${key}`))
    : [prefix];

describe('KeyboardShortcutsDialog', () => {
  beforeEach(() => {
    onPlatform('Win32');
  });

  afterEach(() => {
    vi.restoreAllMocks();
    useDisplayStore.setState({ enterKey: 'send' });
  });

  it('names itself and its groups', () => {
    open();
    expect(screen.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeInTheDocument();
    expect(headings()).toEqual(['Getting around', 'Reading', 'Writing', 'Messages']);
  });

  it('lists every shortcut with its keys', () => {
    open();
    expect(table()).toEqual({
      'Jump to a conversation': 'CtrlK',
      'Previous or next channel': 'Alt↑orAlt↓',
      'Previous or next unread channel': 'AltShift↑orAltShift↓',
      'All unreads': 'AltShiftA',
      'Keyboard shortcuts': 'Ctrl/',
      'Mark the channel read': 'Esc',
      'Mark everything read': 'ShiftEsc',
      'Send the message': 'Enter',
      'Start a new line': 'ShiftEnter',
      'Edit your last messagein an empty message box': '↑',
      'Move between messageson a focused message': '↑or↓',
      Edit: 'E',
      'Open the thread': 'TorEnter',
      'Add a reaction': 'R',
      'Save for later': 'A',
      'Pin or unpin': 'P',
      'Back to the message box': 'Esc',
    });
  });

  it('follows the Enter key setting', () => {
    useDisplayStore.setState({ enterKey: 'newline' });
    open();
    expect(table()['Send the message']).toBe('CtrlEnter');
    expect(table()['Start a new line']).toBe('Enter');
  });

  it('shows Apple keys on a Mac', () => {
    onPlatform('MacIntel');
    open();
    expect(table()['Jump to a conversation']).toBe('⌘K');
    expect(table()['Previous or next unread channel']).toBe('⌥⇧↑or⌥⇧↓');
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('names itself and its groups', () => {
      open();
      expect(screen.getByRole('dialog', { name: 'キーボードショートカット' })).toBeInTheDocument();
      expect(headings()).toEqual(['移動', '既読', '入力', 'メッセージ']);
    });

    it('lists every shortcut with its keys', () => {
      open();
      expect(table()).toEqual({
        チャンネルやメンバーに移動: 'CtrlK',
        前後のチャンネルに移動: 'Alt↑またはAlt↓',
        前後の未読チャンネルに移動: 'AltShift↑またはAltShift↓',
        すべての未読: 'AltShiftA',
        キーボードショートカット: 'Ctrl/',
        チャンネルを既読にする: 'Esc',
        すべて既読にする: 'ShiftEsc',
        送信する: 'Enter',
        改行する: 'ShiftEnter',
        自分が最後に送ったメッセージを編集メッセージの入力欄が空のとき: '↑',
        前後のメッセージに移動メッセージを選んでいるとき: '↑または↓',
        編集: 'E',
        スレッドを開く: 'TまたはEnter',
        リアクションを追加: 'R',
        '「後で」に保存': 'A',
        'ピン留めする・外す': 'P',
        入力欄に戻る: 'Esc',
      });
    });

    it('follows the Enter key setting', () => {
      useDisplayStore.setState({ enterKey: 'newline' });
      open();
      expect(table()['送信する']).toBe('CtrlEnter');
      expect(table()['改行する']).toBe('Enter');
    });

    it('has Japanese for every one', () => {
      const english = flatten(i18n.getResource('en', 'messages', 'shortcuts'), 'shortcuts');
      const japanese = flatten(i18n.getResource('ja', 'messages', 'shortcuts'), 'shortcuts');
      expect(english.length).toBeGreaterThan(0);
      expect(japanese).toEqual(english);
    });
  });

  describe.each(['en', 'ja'])('with text that looks like markup (%s)', (language) => {
    const keys = [
      ...flatten(i18n.getResource('en', 'messages', 'shortcuts'), 'shortcuts').map((key) => [
        'messages',
        key,
      ]),
      ['messages', 'actions.edit'],
      ['messages', 'actions.save'],
      ['sidebar', 'allUnreads'],
    ];
    let saved: string[] = [];

    beforeEach(async () => {
      await i18n.changeLanguage(language);
      saved = keys.map(([ns, key]) => i18n.getResource(language, ns, key) as string);
      keys.forEach(([ns, key]) => i18n.addResource(language, ns, key, hostile));
    });

    afterEach(async () => {
      keys.forEach(([ns, key], i) => i18n.addResource(language, ns, key, saved[i]));
      await i18n.changeLanguage('en');
    });

    it('shows it as text', () => {
      open();
      expect(screen.getByRole('dialog', { name: hostile })).toBeInTheDocument();
      expect(headings()).toEqual([hostile, hostile, hostile, hostile]);
      const labels = Array.from(document.querySelectorAll('dt')).map((dt) => dt.textContent);
      expect(labels).toHaveLength(17);
      expect(labels.filter((label) => label === hostile)).toHaveLength(15);
      expect(labels.filter((label) => label === hostile + hostile)).toHaveLength(2);
      expect(table()[hostile + hostile]).toBe(`↑${hostile}↓`);
      expect(document.querySelector('[onerror]')).toBeNull();
    });

    it('keeps rows apart when their text is the same', () => {
      const error = vi.spyOn(console, 'error').mockImplementation(() => {});
      open();
      expect(error).not.toHaveBeenCalled();
    });
  });
});
