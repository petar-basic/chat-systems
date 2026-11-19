import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import i18n from '@/lib/i18n';
import type { Channel } from '@/stores/workspace';
import { DraftsPanel } from './DraftsPanel';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const sent = vi.hoisted(() => ({ state: {} as Record<string, unknown> }));
vi.mock('./hooks/useSentMessages', () => ({ useSentMessages: () => sent.state }));
vi.mock('@/features/messaging', () => ({ ScheduledList: () => <p data-qa="scheduled-list" /> }));

const channels = [{ id: 'ch-1', name: 'design' }] as unknown as Channel[];

const message = (over: Record<string, unknown> = {}) => ({
  id: 'm-1',
  channel_id: 'ch-1',
  user_id: 'u-me',
  client_message_id: null,
  content: 'hello',
  metadata: {},
  thread_parent_id: null,
  reply_count: 0,
  is_pinned: false,
  created_at: '2026-11-14T09:00:00Z',
  updated_at: '2026-11-14T09:00:00Z',
  deleted_at: null,
  ...over,
});

const answer = (over: Record<string, unknown> = {}) => {
  sent.state = {
    data: { pages: [{ data: [] }] },
    isLoading: false,
    isError: false,
    hasNextPage: false,
    fetchNextPage: vi.fn(),
    isFetchingNextPage: false,
    ...over,
  };
};

function renderPanel() {
  return render(
    <MemoryRouter>
      <DraftsPanel
        workspaceId="ws-1"
        channels={channels}
        conversations={[]}
        onClose={vi.fn()}
        onOpenMessage={vi.fn()}
      />
    </MemoryRouter>,
  );
}

function openSent() {
  renderPanel();
  fireEvent.click(screen.getByTestId('drafts-tab-sent'));
}

describe('DraftsPanel in Japanese', () => {
  beforeEach(async () => {
    answer();
    await i18n.changeLanguage('ja');
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the panel and its tabs', () => {
    renderPanel();
    expect(screen.getByText('下書きと送信済み')).toBeInTheDocument();
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      '下書き',
      '予約済み',
      '送信済み',
    ]);
  });

  it('says nothing was sent yet', () => {
    openSent();
    expect(screen.getByTestId('sent-empty').textContent).toBe(
      '送信したメッセージはまだありません。チャンネル、ダイレクトメッセージ、スレッドで送信したメッセージが、ここに表示されます。',
    );
  });

  it('says what was sent could not be loaded', () => {
    answer({ isError: true });
    openSent();
    expect(screen.getByTestId('sent-error').textContent).toBe('送信済みのメッセージを読み込めませんでした');
  });

  it('shows a sent message as text, and older ones on request', () => {
    answer({
      data: { pages: [{ data: [message({ content: hostile, thread_parent_id: 'm-0' })] }] },
      hasNextPage: true,
    });
    openSent();

    expect(screen.getByTestId('sent-content').textContent).toBe(hostile);
    expect(screen.getByTestId('sent-target')).toHaveTextContent('#design · スレッド');
    expect(screen.getByTestId('sent-more').textContent).toBe('古いメッセージを表示');
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says while older ones load', () => {
    answer({ data: { pages: [{ data: [message()] }] }, hasNextPage: true, isFetchingNextPage: true });
    openSent();
    expect(screen.getByTestId('sent-more').textContent).toBe('読み込み中…');
  });
});

describe('DraftsPanel in English', () => {
  beforeEach(() => answer());

  it('shows older sent messages on request, and says while they load', () => {
    answer({ data: { pages: [{ data: [message()] }] }, hasNextPage: true });
    const { unmount } = renderPanel();
    fireEvent.click(screen.getByTestId('drafts-tab-sent'));
    expect(screen.getByTestId('sent-more').textContent).toBe('Show older');
    unmount();

    answer({ data: { pages: [{ data: [message()] }] }, hasNextPage: true, isFetchingNextPage: true });
    openSent();
    expect(screen.getByTestId('sent-more').textContent).toBe('Loading…');
  });
});

describe('the drafts and sent strings', () => {
  function leaves(value: unknown, prefix = ''): string[] {
    if (typeof value !== 'object' || value === null) return [prefix];
    return Object.entries(value).flatMap(([key, child]) => leaves(child, prefix ? `${prefix}.${key}` : key));
  }

  it.each(['drafts', 'sent'])('has Japanese for every one in %s', (group) => {
    const english = leaves(i18n.getResource('en', 'messages', group));
    const japanese = leaves(i18n.getResource('ja', 'messages', group));
    expect(english.length).toBeGreaterThan(0);
    expect(japanese).toEqual(english);
  });
});
