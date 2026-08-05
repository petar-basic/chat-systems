import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Avatar } from './Avatar';

describe('Avatar', () => {
  it('renders the uploaded image when an avatar url is set', () => {
    render(<Avatar userId="user-1" name="Alice" avatarUrl="/api/files/download/ws/alice.png" />);
    const img = screen.getByRole('img', { name: 'Alice' });
    expect(img).toHaveAttribute('src', '/api/files/download/ws/alice.png');
  });

  it('renders initials when no avatar url is set', () => {
    render(<Avatar userId="user-1" name="Alice" avatarUrl={null} />);
    expect(screen.getByText('A')).toBeInTheDocument();
    expect(screen.queryByRole('img', { name: 'Alice' })?.tagName).not.toBe('IMG');
  });

  it('falls back to initials when the image fails to load', () => {
    render(<Avatar userId="user-1" name="Alice" avatarUrl="/api/files/download/ws/gone.png" />);
    fireEvent.error(screen.getByRole('img', { name: 'Alice' }));
    expect(screen.getByText('A')).toBeInTheDocument();
    expect(document.querySelector('img')).toBeNull();
  });

  it('shows a placeholder initial for an empty name', () => {
    render(<Avatar userId="user-1" name="" />);
    expect(screen.getByText('?')).toBeInTheDocument();
  });

  it('derives a stable colour from the user id', () => {
    const { container: first } = render(<Avatar userId="user-1" name="Alice" />);
    const { container: second } = render(<Avatar userId="user-1" name="Alice" />);
    const colourOf = (el: HTMLElement) =>
      Array.from(el.querySelector('span')?.classList ?? []).find((c) => c.startsWith('bg-'));
    expect(colourOf(first)).toBe(colourOf(second));
  });
});
