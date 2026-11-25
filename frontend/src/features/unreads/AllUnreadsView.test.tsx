import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import en from '@/locales/en/messages.json';
import ja from '@/locales/ja/messages.json';
import { useUserCache } from '@/stores/users';
import type { Conversation } from '@/features/messaging';
import { AllUnreadsView } from './AllUnreadsView';

const mutate = vi.fn();
const fetchNextPage = vi.fn();
const message = {
  id: 'm1',
  channel_id: 'ch-1',
  user_id: 'u2',
  client_message_id: null,
  content: 'first new message',
  metadata: {},
  thread_parent_id: null,
  reply_count: 0,
  is_pinned: false,
  created_at: '2026-10-21T09:00:00Z',
  updated_at: '2026-10-21T09:00:00Z',
  deleted_at: null,
};
const makeGroup = () => ({
  channel_id: 'ch-1',
  channel_name: 'design' as string | null,
  channel_type: 'public' as 'public' | 'private' | 'dm' | 'group_dm',
  muted: false,
  unread_count: 2,
  mention_count: 0,
  last_activity_at: '2026-10-21T10:00:00Z',
  messages: [message],
});
let group = makeGroup();

const state = vi.hoisted(() => ({
  isLoading: false,
  isError: false,
  isSuccess: true,
  empty: false,
  hasNextPage: false,
  isFetchingNextPage: false,
}));

vi.mock('@/features/unreads/hooks/useUnreads', () => ({
  useUnreads: () => ({
    data: { pages: [{ data: state.empty ? [] : [group], next_offset: null }] },
    isLoading: state.isLoading,
    isError: state.isError,
    isSuccess: state.isSuccess,
    hasNextPage: state.hasNextPage,
    isFetchingNextPage: state.isFetchingNextPage,
    fetchNextPage,
  }),
  useMarkGroupRead: () => ({ mutate, isPending: false }),
}));

const onOpenChannel = vi.fn();
const onOpenConversation = vi.fn();

const renderView = (conversations: Conversation[] = []) =>
  render(
    <AllUnreadsView
      workspaceId="ws-1"
      currentUserId="u1"
      conversations={conversations}
      onOpenChannel={onOpenChannel}
      onOpenConversation={onOpenConversation}
      onOpenNav={vi.fn()}
    />,
  );

const dm = (name: string): Conversation[] => {
  useUserCache
    .getState()
    .populateUsers([{ id: 'u3', email: 'u3@example.com', display_name: name, avatar_url: null }]);
  group.channel_type = 'dm';
  group.channel_name = null;
  return [
    {
      id: 'ch-1',
      workspace_id: 'ws-1',
      kind: 'direct',
      participant_ids: ['u1', 'u3'],
      departed: [],
      effective_notify_level: 'all',
      last_message_at: '2026-10-21T10:00:00Z',
      unread_count: 2,
    },
  ];
};

describe('AllUnreadsView', () => {
  beforeEach(() => {
    mutate.mockClear();
    onOpenChannel.mockClear();
    group = makeGroup();
    Object.assign(state, {
      isLoading: false,
      isError: false,
      isSuccess: true,
      empty: false,
      hasNextPage: false,
      isFetchingNextPage: false,
    });
  });

  it('shows each channel with its new messages', () => {
    renderView();
    expect(screen.getByTestId('all-unreads-group')).toHaveTextContent('design');
    expect(screen.getByText('first new message')).toBeInTheDocument();
  });

  it('marks a channel read from its button', () => {
    renderView();
    fireEvent.click(screen.getByTestId('all-unreads-mark-read'));
    expect(mutate).toHaveBeenCalledWith(group);
  });

  it('marks the focused channel read on Escape', () => {
    renderView();
    const section = screen.getByTestId('all-unreads-group');
    section.focus();
    fireEvent.keyDown(section, { key: 'Escape' });
    expect(mutate).toHaveBeenCalledWith(group);
  });

  it('keeps the English wording', () => {
    group.unread_count = 5;
    renderView();
    expect(screen.getByRole('heading', { name: 'All unreads' })).toBeInTheDocument();
    expect(screen.getByRole('main', { name: 'All unreads' })).toBeInTheDocument();
    expect(screen.getAllByText('5 new')).toHaveLength(2);
    expect(screen.getByRole('region', { name: 'design, 5 unread' })).toBeInTheDocument();
    expect(screen.getByTestId('all-unreads-mark-read').textContent).toBe('Mark as read');
    expect(screen.getByText(/4 earlier/).textContent).toBe('4 earlier in the channel');
  });

  describe('in Japanese', () => {
    beforeEach(async () => {
      await i18n.changeLanguage('ja');
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('shows the heading, counts and actions in Japanese', () => {
      group.muted = true;
      renderView();
      expect(screen.getByRole('main', { name: 'すべての未読' })).toBeInTheDocument();
      expect(screen.getByRole('heading', { name: 'すべての未読' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'メニューを開く' })).toHaveAttribute(
        'data-tooltip',
        'メニューを開く',
      );
      expect(screen.getAllByText('新着 2件')).toHaveLength(2);
      expect(screen.getByRole('region', { name: 'design、未読 2件' })).toBeInTheDocument();
      expect(screen.getByLabelText('ミュート中')).toBeInTheDocument();
      expect(screen.getByTestId('all-unreads-mark-read').textContent).toBe('既読にする');
    });

    it('says how many earlier unreads there are and opens them', () => {
      group.unread_count = 5;
      renderView();
      expect(screen.getByText(/それより前の未読/).textContent).toBe('それより前の未読 4件は 開いて見る');
      fireEvent.click(screen.getByRole('button', { name: '開いて見る' }));
      expect(onOpenChannel).toHaveBeenCalledWith('ch-1');
    });

    it('names a channel or direct message it cannot find', () => {
      group.channel_name = null;
      const { unmount } = renderView();
      expect(screen.getByRole('region', { name: 'チャンネル、未読 2件' })).toBeInTheDocument();
      unmount();
      group.channel_type = 'dm';
      renderView();
      expect(screen.getByRole('region', { name: 'ダイレクトメッセージ、未読 2件' })).toBeInTheDocument();
    });

    it('shows loading, failure and an empty list in Japanese', () => {
      state.isLoading = true;
      state.isSuccess = false;
      const { rerender } = renderView();
      expect(screen.getByText('読み込み中…')).toBeInTheDocument();
      Object.assign(state, { isLoading: false, isError: true });
      rerender(
        <AllUnreadsView
          workspaceId="ws-1"
          currentUserId="u1"
          conversations={[]}
          onOpenChannel={onOpenChannel}
          onOpenConversation={onOpenConversation}
        />,
      );
      expect(screen.getByText('未読のメッセージを読み込めませんでした。')).toBeInTheDocument();
      Object.assign(state, { isError: false, isSuccess: true, empty: true });
      rerender(
        <AllUnreadsView
          workspaceId="ws-1"
          currentUserId="u1"
          conversations={[]}
          onOpenChannel={onOpenChannel}
          onOpenConversation={onOpenConversation}
        />,
      );
      expect(screen.getByTestId('all-unreads-empty').textContent).toBe(
        'すべて既読ですチャンネルにもダイレクトメッセージにも、新着メッセージはありません。',
      );
    });

    it('offers more in Japanese and says when it is loading them', () => {
      state.hasNextPage = true;
      const { unmount } = renderView();
      fireEvent.click(screen.getByRole('button', { name: 'さらに表示' }));
      expect(fetchNextPage).toHaveBeenCalled();
      unmount();
      state.isFetchingNextPage = true;
      renderView();
      expect(screen.getByRole('button', { name: '読み込み中…' })).toBeDisabled();
    });

    it('has Japanese for every string', () => {
      expect(Object.keys(ja.unreads).sort()).toEqual(Object.keys(en.unreads).sort());
      for (const value of Object.values(ja.unreads)) expect(value).toMatch(/[぀-ヿ一-鿿]/);
    });
  });

  describe.each(['en', 'ja'])('with a hostile name (%s)', (language) => {
    const hostile = '<img src=x onerror="alert(1)"> {{name}}';
    const label = language === 'ja' ? `${hostile}、未読 2件` : `${hostile}, 2 unread`;

    beforeEach(async () => {
      await i18n.changeLanguage(language);
    });

    afterEach(async () => {
      await i18n.changeLanguage('en');
    });

    it('shows a channel name as text', () => {
      group.channel_name = hostile;
      renderView();
      expect(screen.getByRole('button', { name: hostile }).textContent).toBe(hostile);
      expect(screen.getByTestId('all-unreads-group')).toHaveAttribute('aria-label', label);
      expect(document.querySelector('[onerror]')).toBeNull();
    });

    it('shows a direct message name as text', () => {
      renderView(dm(hostile));
      expect(screen.getByRole('button', { name: hostile }).textContent).toBe(hostile);
      expect(screen.getByTestId('all-unreads-group')).toHaveAttribute('aria-label', label);
      expect(document.querySelector('[onerror]')).toBeNull();
    });
  });
});
