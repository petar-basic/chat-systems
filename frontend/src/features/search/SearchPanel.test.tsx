import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import SearchPanel from './SearchPanel';
import { useWorkspaceStore } from '@/stores/workspace';

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
    fireEvent.click(screen.getByTitle('Show in conversation'));
    expect(onOpenMessage).toHaveBeenCalledWith('m-1');
    fireEvent.click(screen.getByTitle('Open in the gallery'));
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
