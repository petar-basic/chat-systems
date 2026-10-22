import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import type { Message } from '@/stores/workspace';
import { GalleryProvider } from './GalleryProvider';
import { MessageAttachments } from './MessageAttachments';

const image = (id: string) => ({
  id,
  filename: `${id}.png`,
  mime_type: 'image/png',
  size_bytes: 10,
  url: `https://chat.test/api/files/download/ws/${id}.png`,
});

const message = (id: string, attachments: ReturnType<typeof image>[]): Message => ({
  id,
  channel_id: 'ch',
  user_id: 'u-1',
  client_message_id: null,
  content: '',
  created_at: '2026-10-22T10:00:00Z',
  updated_at: '2026-10-22T10:00:00Z',
  deleted_at: null,
  thread_parent_id: null,
  reply_count: 0,
  is_pinned: false,
  metadata: { attachments },
});

function Conversation() {
  const messages = [message('m1', [image('a'), image('b')]), message('m2', [image('c')])];
  return (
    <GalleryProvider messages={messages}>
      {messages.map((m) => (
        <MessageAttachments key={m.id} attachments={m.metadata?.attachments ?? []} />
      ))}
    </GalleryProvider>
  );
}

const shown = () => screen.getByTestId('gallery-image').getAttribute('alt');

describe('ImageGallery', () => {
  it('steps through every image in the conversation, across messages', () => {
    render(<Conversation />);
    fireEvent.click(screen.getAllByTestId('attachment-image')[1]);
    expect(shown()).toBe('b.png');
    expect(screen.getByTestId('gallery-counter')).toHaveTextContent('2 / 3');

    fireEvent.keyDown(window, { key: 'ArrowRight' });
    expect(shown()).toBe('c.png');
    expect(screen.queryByTestId('gallery-next')).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId('gallery-previous'));
    fireEvent.keyDown(window, { key: 'ArrowLeft' });
    expect(shown()).toBe('a.png');
    expect(screen.queryByTestId('gallery-previous')).not.toBeInTheDocument();
  });

  it('zooms on click and closes on Escape', () => {
    render(<Conversation />);
    fireEvent.click(screen.getAllByTestId('attachment-image')[0]);
    const img = screen.getByTestId('gallery-image');
    expect(img).toHaveClass('cursor-zoom-in');
    fireEvent.click(img);
    expect(screen.getByTestId('gallery-image')).toHaveClass('cursor-zoom-out');
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByTestId('image-lightbox')).not.toBeInTheDocument();
  });

  it('works on its own outside a conversation', () => {
    render(<MessageAttachments attachments={[image('x'), image('y')]} />);
    fireEvent.click(screen.getAllByTestId('attachment-image')[1]);
    expect(shown()).toBe('y.png');
    expect(screen.getByTestId('gallery-counter')).toHaveTextContent('2 / 2');
  });
});
