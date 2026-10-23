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
vi.mock('@/hooks/queries/useWorkspaces', () => ({
  useWorkspaceChannels: () => ({ data: [{ id: 'c-1', name: 'finance', channel_type: 'public' }] }),
}));
vi.mock('@/hooks/queries/useConversations', () => ({
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
  user_id: 'u-1',
  created_at: '2026-10-22T10:00:00Z',
});

const files = [
  hit('1', 'Q3 budget.pdf', 'application/pdf', 'pdf'),
  hit('2', 'budget-chart.png', 'image/png', 'image'),
];

function renderPanel() {
  const onNavigateToMessage = vi.fn();
  render(<SearchPanel members={[]} onClose={vi.fn()} onNavigateToMessage={onNavigateToMessage} />);
  return onNavigateToMessage;
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
    const onNavigateToMessage = renderPanel();
    fireEvent.click(screen.getByTestId('search-tab-files'));
    typeAndSubmit('budget');
    await waitFor(() => expect(screen.getAllByTestId('file-hit')).toHaveLength(2));
    fireEvent.click(screen.getByTitle('Show in conversation'));
    expect(onNavigateToMessage).toHaveBeenCalledWith('c-1', 'm-1');
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
});
