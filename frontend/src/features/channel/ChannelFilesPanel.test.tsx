import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import ChannelFilesPanel from './ChannelFilesPanel';

const file = (id: string, filename: string, mime_type: string) => ({
  id,
  filename,
  mime_type,
  size_bytes: 2048,
  url: `https://chat.test/api/files/download/ws/${id}/${filename}`,
  message_id: `m-${id}`,
  user_id: 'u-1',
  created_at: '2026-10-22T10:00:00Z',
});

const kinds: (string | null)[] = [];
const names: string[] = [];
vi.mock('@/features/channel/hooks/useChannelFiles', () => ({
  useChannelFileSearch: (_workspaceId: string | undefined, _channelId: string, name: string) => {
    names.push(name);
    return {
      data: name.startsWith('<')
        ? []
        : name
          ? [
              { ...file('3', 'Q3 budget.xlsx', 'application/vnd.ms-excel'), kind: 'spreadsheet' },
              { ...file('4', 'budget-chart.png', 'image/png'), kind: 'image' },
            ]
          : undefined,
      isLoading: false,
      isError: false,
    };
  },
  useChannelFiles: (_channelId: string, kind: string | null) => {
    kinds.push(kind);
    return {
      data: {
        pages: [{ data: [file('1', 'shot.png', 'image/png'), file('2', 'plan.pdf', 'application/pdf')] }],
      },
      isLoading: false,
      isError: false,
      hasNextPage: false,
      fetchNextPage: vi.fn(),
      isFetchingNextPage: false,
    };
  },
}));

describe('ChannelFilesPanel', () => {
  beforeEach(() => {
    kinds.length = 0;
    names.length = 0;
  });

  it('shows images as a grid and other files as rows that lead back to the message', () => {
    const onNavigate = vi.fn();
    render(<ChannelFilesPanel channelId="ch" onClose={vi.fn()} onNavigate={onNavigate} />);
    expect(screen.getByTestId('channel-files-images').querySelectorAll('img')).toHaveLength(1);
    const row = screen.getByTestId('channel-file-row');
    expect(row).toHaveTextContent('plan.pdf');
    expect(row).toHaveTextContent('2 KB');
    fireEvent.click(document.querySelector('[data-tooltip="Show in conversation"]')!);
    expect(onNavigate).toHaveBeenCalledWith('m-2');
  });

  it('filters by kind and opens images in the gallery', () => {
    render(<ChannelFilesPanel channelId="ch" onClose={vi.fn()} onNavigate={vi.fn()} />);
    fireEvent.click(screen.getByTestId('channel-files-tab-document'));
    expect(kinds.at(-1)).toBe('document');
    fireEvent.click(screen.getByRole('button', { name: 'shot.png' }));
    expect(screen.getByTestId('image-lightbox')).toBeInTheDocument();
  });

  it('filters by name through search and keeps the kind tab', async () => {
    render(<ChannelFilesPanel channelId="ch" onClose={vi.fn()} onNavigate={vi.fn()} />);
    fireEvent.change(screen.getByLabelText('Filter files by name'), { target: { value: ' budg ' } });
    await waitFor(() => expect(names.at(-1)).toBe('budg'));
    expect(screen.getByTestId('channel-file-row')).toHaveTextContent('Q3 budget.xlsx');
    expect(screen.getByRole('button', { name: 'budget-chart.png' })).toBeInTheDocument();
    expect(screen.queryByText('plan.pdf')).toBeNull();
    fireEvent.click(screen.getByTestId('channel-files-tab-document'));
    expect(screen.queryByRole('button', { name: 'budget-chart.png' })).toBeNull();
    expect(screen.getByTestId('channel-file-row')).toHaveTextContent('Q3 budget.xlsx');
  });
});

describe('ChannelFilesPanel in Japanese', () => {
  const hostile = '<img src=x onerror="alert(1)"> {{name}}';
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the tabs and the files to download', () => {
    render(<ChannelFilesPanel channelId="ch" onClose={vi.fn()} onNavigate={vi.fn()} />);
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      'すべて',
      '画像',
      '動画',
      '音声',
      '文書',
    ]);
    expect(screen.getByRole('link', { name: 'plan.pdf をダウンロード' })).toBeInTheDocument();
  });

  it('repeats a name that matched nothing, as text', async () => {
    render(<ChannelFilesPanel channelId="ch" onClose={vi.fn()} onNavigate={vi.fn()} />);
    fireEvent.change(screen.getByLabelText('ファイル名で絞り込む'), { target: { value: hostile } });
    expect(await screen.findByText(`「${hostile}」に一致するファイルはありません`)).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
