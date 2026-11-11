import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, fireEvent } from '@testing-library/react';
import ChannelBookmarksBar from './ChannelBookmarksBar';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

vi.mock('@/features/channel/hooks/useChannelBookmarks', () => ({
  useChannelBookmarks: () => ({
    data: [{ id: 'b1', label: hostile, url: 'https://example.test', emoji: null }],
  }),
  useCreateChannelBookmark: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useDeleteChannelBookmark: () => ({ mutate: vi.fn() }),
}));
vi.mock('./hooks/useChannelModeration', () => ({ useChannelModeration: () => ({ canModerate: true }) }));
vi.mock('@/lib/channelArchive', () => ({ useChannelArchived: () => false }));
vi.mock('@/features/lists', () => ({ ChannelListsMenu: () => null }));

describe('ChannelBookmarksBar in Japanese', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names a bookmark to remove as text, and asks for both a label and a link', () => {
    const { rerender } = render(
      <ChannelBookmarksBar workspaceId="w1" channelId="c1" canPost adding={false} onAddingChange={vi.fn()} />,
    );
    expect(screen.getByRole('button', { name: `ブックマーク「${hostile}」を削除` })).toBeInTheDocument();
    expect(screen.getByTestId('channel-bookmark-add')).toHaveTextContent('ブックマークを追加');

    rerender(<ChannelBookmarksBar workspaceId="w1" channelId="c1" canPost adding onAddingChange={vi.fn()} />);
    expect(screen.getByRole('textbox', { name: 'ブックマーク名' })).toHaveAttribute('placeholder', '名前');
    fireEvent.click(screen.getByTestId('channel-bookmark-save'));
    expect(screen.getByTestId('channel-bookmark-error').textContent).toBe(
      '名前とリンクの両方を入力してください',
    );
    expect(screen.getByRole('button', { name: 'キャンセル' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
