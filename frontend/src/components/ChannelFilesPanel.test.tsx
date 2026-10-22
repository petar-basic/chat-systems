import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
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
vi.mock('@/hooks/queries/useChannelFiles', () => ({
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
  });

  it('shows images as a grid and other files as rows that lead back to the message', () => {
    const onNavigate = vi.fn();
    render(<ChannelFilesPanel channelId="ch" onClose={vi.fn()} onNavigate={onNavigate} />);
    expect(screen.getByTestId('channel-files-images').querySelectorAll('img')).toHaveLength(1);
    const row = screen.getByTestId('channel-file-row');
    expect(row).toHaveTextContent('plan.pdf');
    expect(row).toHaveTextContent('2 KB');
    fireEvent.click(screen.getByTitle('Show in conversation'));
    expect(onNavigate).toHaveBeenCalledWith('m-2');
  });

  it('filters by kind and opens images in the gallery', () => {
    render(<ChannelFilesPanel channelId="ch" onClose={vi.fn()} onNavigate={vi.fn()} />);
    fireEvent.click(screen.getByTestId('channel-files-tab-document'));
    expect(kinds.at(-1)).toBe('document');
    fireEvent.click(screen.getByTitle('shot.png'));
    expect(screen.getByTestId('image-lightbox')).toBeInTheDocument();
  });
});
