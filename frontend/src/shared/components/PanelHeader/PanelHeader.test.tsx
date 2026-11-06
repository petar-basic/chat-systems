import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { PanelHeader } from './PanelHeader';

describe('PanelHeader', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('in Japanese names the close button after the panel, as text', async () => {
    const title = '<img src=x onerror="alert(1)"> {{name}}';
    await i18n.changeLanguage('ja');
    render(<PanelHeader title={title} onClose={vi.fn()} />);
    expect(screen.getByRole('heading', { name: title })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: `${title}を閉じる` })).toBeInTheDocument();
    expect(document.querySelector('img')).toBeNull();
  });

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
