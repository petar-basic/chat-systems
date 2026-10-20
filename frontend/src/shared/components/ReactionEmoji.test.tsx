import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ReactionEmoji } from './ReactionEmoji';
import { useCustomEmojiStore } from '@/stores/customEmoji';

describe('ReactionEmoji', () => {
  beforeEach(() => {
    useCustomEmojiStore.getState().populate([
      {
        id: 'e1',
        name: 'shipit',
        url: '/api/files/download/emoji/shipit.png',
        created_by: 'u1',
        created_at: '2024-01-01T00:00:00Z',
      },
    ]);
  });

  it('renders a unicode reaction as itself', () => {
    render(<ReactionEmoji emoji="🎉" />);
    expect(screen.getByText('🎉')).toBeInTheDocument();
  });

  it('renders an imported shortcode as the image it stands for', () => {
    render(<ReactionEmoji emoji=":shipit:" />);
    const img = screen.getByRole('img', { name: ':shipit:' });
    expect(img).toHaveAttribute('src', '/api/files/download/emoji/shipit.png');
  });

  it('leaves a shortcode nobody uploaded as text', () => {
    render(<ReactionEmoji emoji=":nothing:" />);
    expect(screen.getByText(':nothing:')).toBeInTheDocument();
  });
});
