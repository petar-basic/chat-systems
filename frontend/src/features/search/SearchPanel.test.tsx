import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import SearchPanel from './SearchPanel';
import { useWorkspaceStore } from '@/stores/workspace';
import type { RecentSearch } from './useRecentSearches';

const get = vi.fn();
vi.mock('@/shared/hooks/useCurrentApi', () => ({
  getApiForInstance: () => ({
    typed: (op: (client: unknown) => Promise<{ data: unknown }>) =>
      op({
        GET: async (path: string, init?: { params?: { query?: unknown } }) => ({
          data: await get(path, init?.params?.query),
        }),
      }).then((result) => result.data),
  }),
}));
vi.mock('@/features/workspace/hooks/useWorkspaces', () => ({
  useWorkspaceChannels: () => ({ data: [{ id: 'c-1', name: 'finance', channel_type: 'public' }] }),
}));
vi.mock('@/features/messaging/hooks/useConversations', () => ({
  useConversations: () => ({ data: [] }),
}));
const recent = {
  searches: [] as RecentSearch[],
  record: vi.fn(),
  forget: vi.fn(),
  clear: vi.fn(),
};
vi.mock('./useRecentSearches', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./useRecentSearches')>()),
  useRecentSearches: () => ({ data: recent.searches }),
  useRecentSearchActions: () => ({ record: recent.record, forget: recent.forget, clear: recent.clear }),
}));

const hit = (id: string, filename: string, mime_type: string, kind: string) => ({
  id,
  filename,
  mime_type,
  kind,
  size_bytes: 2048,
  url: `https://chat.test/api/files/download/ws/${id}/${filename}`,
  message_id: `m-${id}`,
  channel_id: 'c-1',
  channel_name: 'finance',
  user_id: 'u-1',
  created_at: '2026-10-22T10:00:00Z',
});

const files = [
  hit('1', 'Q3 budget.pdf', 'application/pdf', 'pdf'),
  hit('2', 'budget-chart.png', 'image/png', 'image'),
];

function renderPanel() {
  const onOpenMessage = vi.fn();
  render(<SearchPanel members={[]} onClose={vi.fn()} onOpenMessage={onOpenMessage} />);
  return onOpenMessage;
}

function typeAndSubmit(text: string) {
  const input = screen.getByRole('textbox');
  fireEvent.change(input, { target: { value: text } });
  fireEvent.keyDown(input, { key: 'Enter' });
}

describe('SearchPanel', () => {
  beforeEach(() => {
    recent.searches = [];
    recent.record.mockReset();
    get.mockReset();
    get.mockImplementation(async (path: string) => ({ data: path === '/search/files' ? files : [] }));
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', name: 'WS' } as never,
      currentUserId: 'u-me',
    });
  });

  it('searches files by name on the Files tab', async () => {
    renderPanel();
    fireEvent.click(screen.getByTestId('search-tab-files'));
    expect(screen.getByTestId('search-tab-files')).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByTestId('search-filter-type')).toBeInTheDocument();
    expect(screen.queryByTestId('search-filter-has')).toBeNull();
    typeAndSubmit('budg');
    await waitFor(() => expect(screen.getAllByTestId('file-hit')).toHaveLength(2));
    expect(get).toHaveBeenLastCalledWith('/search/files', expect.objectContaining({ q: 'budg' }));
    const [pdf] = screen.getAllByTestId('file-hit');
    expect(pdf).toHaveTextContent('Q3 budget.pdf');
    expect(pdf).toHaveTextContent('2 KB');
    expect(pdf).toHaveTextContent('#finance');
    expect(screen.getAllByTestId('file-hit-match')[0]).toHaveTextContent('budg');
    expect(screen.getByTestId('file-hit-kind-pdf')).toBeInTheDocument();
    expect(screen.getByLabelText('Download Q3 budget.pdf')).toHaveAttribute('href', files[0].url);
  });

  it('shows the words found inside a file, marked but never as markup', async () => {
    get.mockImplementation(async () => ({
      data: [
        {
          ...hit('3', 'Agreement_v7.pdf', 'application/pdf', 'pdf'),
          snippet: [
            { text: 'Klijent: <b>Đorđe</b> ', hit: false },
            { text: 'Šćepanović', hit: true },
            { text: ', Čačak', hit: false },
          ],
        },
      ],
    }));
    renderPanel();
    fireEvent.click(screen.getByTestId('search-tab-files'));
    typeAndSubmit('scepanovic');
    const snippet = await screen.findByTestId('file-hit-snippet');
    expect(snippet).toHaveTextContent('Klijent: <b>Đorđe</b> Šćepanović, Čačak');
    expect(snippet.querySelector('b')).toBeNull();
    expect(screen.getByTestId('file-hit-snippet-match')).toHaveTextContent('Šćepanović');
    expect(screen.queryByTestId('file-hit-match')).toBeNull();
  });

  it('switches to Files when type: is typed', async () => {
    renderPanel();
    typeAndSubmit('budget type:pdf');
    await waitFor(() =>
      expect(get).toHaveBeenLastCalledWith(
        '/search/files',
        expect.objectContaining({ q: 'budget', kind: 'pdf' }),
      ),
    );
    expect(screen.getByTestId('search-tab-files')).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByTestId('search-filter-type')).toHaveTextContent('PDF');
  });

  it('opens an image in the gallery and any other file at its message', async () => {
    const onOpenMessage = renderPanel();
    fireEvent.click(screen.getByTestId('search-tab-files'));
    typeAndSubmit('budget');
    await waitFor(() => expect(screen.getAllByTestId('file-hit')).toHaveLength(2));
    fireEvent.click(document.querySelector('[data-tooltip="Show in conversation"]')!);
    expect(onOpenMessage).toHaveBeenCalledWith('m-1');
    fireEvent.click(document.querySelector('[data-tooltip="Open in the gallery"]')!);
    expect(screen.getByTestId('image-lightbox')).toBeInTheDocument();
  });

  it('keeps messages on the Messages tab', async () => {
    renderPanel();
    typeAndSubmit('budget');
    await waitFor(() =>
      expect(get).toHaveBeenLastCalledWith('/search', expect.objectContaining({ q: 'budget' })),
    );
    expect(screen.queryByTestId('file-hit')).toBeNull();
  });

  it('shows more messages past the first page', async () => {
    const message = (n: number) => ({
      id: `m-${n}`,
      channel_id: 'c-1',
      channel_name: 'finance',
      user_id: 'u-1',
      content: `budget ${n}`,
      created_at: '2026-10-22T10:00:00Z',
    });
    get.mockImplementation(async (_path: string, query: { offset: number }) => ({
      data:
        query.offset === 0 ? Array.from({ length: 20 }, (_, n) => message(n)) : [message(20), message(21)],
    }));
    renderPanel();
    typeAndSubmit('budget');
    await waitFor(() => expect(screen.getAllByTestId('search-result')).toHaveLength(20));
    expect(screen.getByTestId('search-summary')).toHaveTextContent('20+ results');

    fireEvent.click(screen.getByTestId('search-show-more'));

    await waitFor(() => expect(screen.getAllByTestId('search-result')).toHaveLength(22));
    expect(get).toHaveBeenLastCalledWith('/search', expect.objectContaining({ q: 'budget', offset: 20 }));
    expect(screen.getByTestId('search-summary')).toHaveTextContent('22 results');
    expect(screen.queryByTestId('search-show-more')).toBeNull();
  });

  it('names a channel the person is not in', async () => {
    get.mockImplementation(async () => ({
      data: [
        {
          id: 'm-elsewhere',
          channel_id: 'c-9',
          channel_name: 'design',
          user_id: 'u-1',
          content: 'budget review',
          created_at: '2026-10-22T10:00:00Z',
        },
      ],
    }));
    renderPanel();
    typeAndSubmit('budget');
    await waitFor(() => expect(screen.getByTestId('search-result')).toHaveTextContent('#design'));
  });
});

describe('SearchPanel recent searches', () => {
  const kept = (over: Partial<RecentSearch>): RecentSearch => ({
    id: 'r-1',
    query: 'deploy',
    tab: 'messages',
    filters: {},
    searched_at: '2026-11-16T09:00:00Z',
    ...over,
  });

  beforeEach(() => {
    get.mockReset();
    get.mockImplementation(async () => ({ data: [] }));
    recent.record.mockReset();
    recent.forget.mockReset();
    recent.clear.mockReset();
    recent.searches = [
      kept({ id: 'r-1', query: 'deploy', filters: { in: 'c-1', has: 'link' } }),
      kept({ id: 'r-2', query: 'report', tab: 'files', filters: { kind: 'pdf', on: '2026-11-10' } }),
    ];
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', name: 'WS' } as never,
      currentUserId: 'u-me',
    });
  });

  it('lists them while the box is empty, with their filters as chips', () => {
    renderPanel();
    const rows = screen.getAllByTestId('recent-search');
    expect(rows.map((row) => row.querySelector('[data-qa="recent-search-query"]')?.textContent)).toEqual([
      'deploy',
      'report',
    ]);
    expect(
      Array.from(rows[0].querySelectorAll('[data-qa="recent-search-chip"]')).map((chip) => chip.textContent),
    ).toEqual(['In #finance', 'Links']);
    expect(rows[1]).toHaveTextContent('Files');
    expect(rows[1]).toHaveTextContent('PDF');
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'dep' } });
    expect(screen.queryByTestId('recent-searches')).toBeNull();
  });

  it('runs a picked search with its filters and its tab, and keeps it at the top', async () => {
    renderPanel();
    fireEvent.click(screen.getAllByTestId('recent-search')[1]);
    await waitFor(() =>
      expect(get).toHaveBeenLastCalledWith(
        '/search/files',
        expect.objectContaining({ q: 'report', kind: 'pdf' }),
      ),
    );
    expect(screen.getByRole('textbox')).toHaveValue('report');
    expect(screen.getByTestId('search-tab-files')).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByTestId('search-filter-type')).toHaveTextContent('PDF');
    expect(recent.record).toHaveBeenCalledWith({
      query: 'report',
      tab: 'files',
      filters: { kind: 'pdf', on: '2026-11-10' },
    });
  });

  it('moves through them with the arrow keys and back up to the box', () => {
    renderPanel();
    const input = screen.getByRole('textbox');
    const rows = screen.getAllByTestId('recent-search');
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(rows[0]).toHaveFocus();
    fireEvent.keyDown(rows[0], { key: 'ArrowDown' });
    expect(rows[1]).toHaveFocus();
    fireEvent.keyDown(rows[1], { key: 'ArrowDown' });
    expect(rows[1]).toHaveFocus();
    fireEvent.keyDown(rows[1], { key: 'ArrowUp' });
    fireEvent.keyDown(rows[0], { key: 'ArrowUp' });
    expect(input).toHaveFocus();
  });

  it('removes one, or all of them', () => {
    renderPanel();
    fireEvent.click(screen.getByRole('button', { name: 'Remove “deploy” from recent searches' }));
    expect(recent.forget).toHaveBeenCalledWith('r-1');
    fireEvent.click(screen.getByTestId('recent-searches-clear'));
    expect(recent.clear).toHaveBeenCalled();
  });

  it('keeps a search when it is submitted or a result is opened, not while it is typed', async () => {
    get.mockImplementation(async (path: string) => ({
      data:
        path === '/search'
          ? [
              {
                id: 'm-1',
                channel_id: 'c-1',
                channel_name: 'finance',
                user_id: 'u-1',
                content: 'budget review',
                created_at: '2026-10-22T10:00:00Z',
              },
            ]
          : [],
    }));
    const onOpenMessage = renderPanel();
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'budget in:#finance ' } });
    expect(recent.record).not.toHaveBeenCalled();
    fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Enter' });
    expect(recent.record).toHaveBeenLastCalledWith({
      query: 'budget',
      tab: 'messages',
      filters: { in: 'c-1' },
    });
    fireEvent.click(await screen.findByTestId('search-result'));
    expect(onOpenMessage).toHaveBeenCalledWith('m-1');
    expect(recent.record).toHaveBeenCalledTimes(2);
  });
});

describe('SearchPanel in Japanese', () => {
  const hostile = '<img src=x onerror="alert(1)"> {{name}}';
  beforeEach(async () => {
    recent.searches = [];
    get.mockReset();
    get.mockImplementation(async (path: string) => ({
      data: path === '/search/files' ? [hit('9', hostile, 'application/pdf', 'pdf')] : [],
    }));
    useWorkspaceStore.setState({
      currentWorkspace: { id: 'ws-1', name: 'WS' } as never,
      currentUserId: 'u-me',
    });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('explains how to narrow a search before anything is typed', () => {
    renderPanel();
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual(['メッセージ', 'ファイル']);
    expect(screen.getByRole('textbox', { name: 'メッセージを検索' })).toBeInTheDocument();
    const tip = screen.getByText(
      (_, el) => el?.tagName === 'P' && !!el.textContent?.startsWith('上の絞り込み'),
    );
    expect(tip.textContent).toBe(
      '上の絞り込みを使うか、from:@名前、in:#チャンネル、has:file、after:2026-10-01 のように入力して絞り込めます',
    );
    expect(tip.querySelectorAll('code')).toHaveLength(4);
  });

  it('repeats a search that found nothing, as text', async () => {
    renderPanel();
    typeAndSubmit(hostile);
    expect(await screen.findByText(`「${hostile}」に一致するメッセージはありません`)).toBeInTheDocument();
    expect(screen.getByTestId('search-summary').textContent).toBe('0 件');
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('names a file to download as text', async () => {
    renderPanel();
    fireEvent.click(screen.getByTestId('search-tab-files'));
    typeAndSubmit('budget');
    expect(await screen.findByRole('link', { name: `${hostile} をダウンロード` })).toBeInTheDocument();
    expect(screen.getByTestId('search-summary').textContent).toBe('1 件');
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
