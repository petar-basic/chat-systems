import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { MessageAttachments } from './MessageAttachments';

const image = (n: number) => ({
  id: `img-${n}`,
  filename: `shot-${n}.png`,
  mime_type: 'image/png',
  size_bytes: 1000,
  url: `https://chat.test/api/files/download/ws/${n}/shot-${n}.png`,
});

const pdf = {
  id: 'pdf-1',
  filename: 'Izveštaj o radu.pdf',
  mime_type: 'application/pdf',
  size_bytes: 1536 * 1024,
  url: 'https://chat.test/api/files/download/ws/9/report.pdf',
};

describe('MessageAttachments', () => {
  it('shows one image on its own and opens it', () => {
    render(<MessageAttachments attachments={[image(1)]} />);
    expect(screen.queryByTestId('attachment-grid')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId('attachment-image'));
    expect(screen.getByTestId('image-lightbox')).toBeInTheDocument();
  });

  it('puts several images in a grid and counts the ones it does not show', () => {
    render(<MessageAttachments attachments={[1, 2, 3, 4, 5, 6].map(image)} />);
    const grid = screen.getByTestId('attachment-grid');
    expect(within(grid).getAllByTestId('attachment-image')).toHaveLength(4);
    expect(screen.getByTestId('attachment-grid-more')).toHaveTextContent('+2');
  });

  it('gives other files a card with their kind and size', () => {
    render(<MessageAttachments attachments={[pdf]} />);
    const card = screen.getByTestId('attachment-file');
    expect(card).toHaveTextContent('Izveštaj o radu.pdf');
    expect(card).toHaveTextContent('PDF · 1.5 MB');
    expect(card).toHaveAttribute('download', 'Izveštaj o radu.pdf');
  });

  it('lists names only when compact', () => {
    render(<MessageAttachments attachments={[image(1), pdf]} compact />);
    expect(screen.getByTestId('message-attachments-compact')).toHaveTextContent('shot-1.png');
    expect(screen.queryByRole('img')).not.toBeInTheDocument();
  });

  it('lets an edit drop a file', () => {
    const onRemove = vi.fn();
    render(<MessageAttachments attachments={[image(1), pdf]} onRemove={onRemove} />);
    fireEvent.click(screen.getByLabelText('Remove shot-1.png'));
    expect(onRemove).toHaveBeenCalledWith('img-1');
  });
});

describe('MessageAttachments image boxes', () => {
  it('reserves the scaled box and shows the thumbnail before the original', () => {
    render(
      <MessageAttachments
        attachments={[
          {
            ...image(7),
            width: 1600,
            height: 900,
            thumbnail_url: 'https://chat.test/api/files/download/ws/t/thumbnail.jpg',
          },
        ]}
      />,
    );
    const img = screen.getByRole('img');
    expect(img).toHaveAttribute('src', 'https://chat.test/api/files/download/ws/t/thumbnail.jpg');
    expect(img).toHaveAttribute('width', '360');
    expect(img).toHaveAttribute('height', '203');
    fireEvent.click(screen.getByTestId('attachment-image'));
    expect(within(screen.getByTestId('image-lightbox')).getByRole('img')).toHaveAttribute(
      'src',
      image(7).url,
    );
  });

  it('plays a gif from the original, not its still thumbnail', () => {
    render(
      <MessageAttachments
        attachments={[{ ...image(8), mime_type: 'image/gif', thumbnail_url: 'https://chat.test/still.jpg' }]}
      />,
    );
    expect(screen.getByRole('img')).toHaveAttribute('src', image(8).url);
  });
});
