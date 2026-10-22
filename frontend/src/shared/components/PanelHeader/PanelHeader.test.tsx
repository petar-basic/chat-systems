import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { PanelHeader } from './PanelHeader';

describe('PanelHeader', () => {
  it('titles the panel and closes it from a labelled button', () => {
    const onClose = vi.fn();
    render(<PanelHeader title="Pinned messages" onClose={onClose} />);
    expect(screen.getByRole('heading', { name: 'Pinned messages' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Close pinned messages' }));
    expect(onClose).toHaveBeenCalled();
  });

  it('takes a close label of its own', () => {
    render(<PanelHeader title="#design members" closeLabel="Close members" onClose={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Close members' })).toBeInTheDocument();
  });
});
