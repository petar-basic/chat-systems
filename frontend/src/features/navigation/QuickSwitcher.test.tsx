import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import type { GlobalShortcut } from '@/features/apps';
import { QuickSwitcher } from './QuickSwitcher';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

let shortcuts: GlobalShortcut[] = [];

vi.mock('@/features/apps', async (original) => ({
  ...(await original<typeof import('@/features/apps')>()),
  useGlobalShortcuts: () => shortcuts,
  useRunAppShortcut: () => ({ mutateAsync: vi.fn() }),
}));

const channel = (name: string | null) =>
  ({ id: `c-${name}`, name, channel_type: 'public' }) as unknown as Channel;
const person = (user_id: string, display_name: string, title?: string) =>
  ({ user_id, display_name, title }) as unknown as WorkspaceMember;

const open = (channels: Channel[], members: WorkspaceMember[] = []) =>
  render(
    <QuickSwitcher
      channels={channels}
      members={members}
      currentUserId="me"
      onSelectChannel={vi.fn()}
      onSelectDm={vi.fn()}
      onClose={vi.fn()}
    />,
  );

const rows = () => screen.getAllByRole('button').map((b) => b.textContent);

describe('QuickSwitcher', () => {
  beforeEach(() => {
    shortcuts = [];
  });

  it('names itself and its search box', () => {
    open([]);
    expect(screen.getByRole('dialog', { name: 'Quick switcher' })).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'Search channels and people' })).toHaveAttribute(
      'placeholder',
      'Jump to a channel or person…',
    );
  });

  it('labels channels and people', () => {
    open([channel('general')], [person('ana', 'Ana'), person('bo', 'Bo', 'Designer')]);
    expect(rows()).toEqual(['generalChannel', 'AnaDirect message', 'BoDesigner']);
  });

  it('names a channel that has no name', () => {
    open([channel(null)]);
    expect(rows()).toEqual(['channelChannel']);
  });

  it('says when nothing matches', () => {
    open([channel('general')]);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'zzz' } });
    expect(screen.getByText('No matches')).toBeInTheDocument();
  });

  it('shows names as text', () => {
    shortcuts = [
      { shortcut: { id: 's1', name: hostile }, app: { name: hostile } } as unknown as GlobalShortcut,
    ];
    open([channel(hostile)], [person('ana', hostile, hostile)]);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: '<img' } });
    expect(rows()).toEqual([`${hostile}Channel`, `${hostile}${hostile}`, `${hostile}${hostile}`]);
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('names itself and its search box', () => {
      open([]);
      expect(screen.getByRole('dialog', { name: 'クイックスイッチャー' })).toBeInTheDocument();
      expect(screen.getByRole('textbox', { name: 'チャンネルやメンバーを検索' })).toHaveAttribute(
        'placeholder',
        'チャンネルやメンバーに移動…',
      );
    });

    it('labels channels and people', () => {
      open([channel('general'), channel(null)], [person('ana', 'Ana'), person('bo', 'Bo', 'Designer')]);
      expect(rows()).toEqual([
        'generalチャンネル',
        'チャンネルチャンネル',
        'Anaダイレクトメッセージ',
        'BoDesigner',
      ]);
    });

    it('says when nothing matches', () => {
      open([channel('general')]);
      fireEvent.change(screen.getByRole('textbox'), { target: { value: 'zzz' } });
      expect(screen.getByText('一致するものがありません')).toBeInTheDocument();
    });

    it('has Japanese for every one', () => {
      const english = Object.keys(i18n.getResource('en', 'messages', 'quickSwitcher'));
      const japanese = Object.keys(i18n.getResource('ja', 'messages', 'quickSwitcher'));
      expect(english.length).toBeGreaterThan(0);
      expect(japanese).toEqual(english);
    });
  });
});
