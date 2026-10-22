import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { X } from 'lucide-react';
import { IconButton } from './IconButton';

describe('IconButton', () => {
  it('names itself for screen readers and as a tooltip', () => {
    render(<IconButton label="Close thread" icon={<X />} />);
    const button = screen.getByRole('button', { name: 'Close thread' });
    expect(button).toHaveAttribute('title', 'Close thread');
    expect(button.className).toContain('h-7 w-7');
  });

  it('reports a pressed toggle', () => {
    render(<IconButton label="Pinned messages" icon={<X />} pressed />);
    expect(screen.getByRole('button', { name: 'Pinned messages' })).toHaveAttribute('aria-pressed', 'true');
  });
});
