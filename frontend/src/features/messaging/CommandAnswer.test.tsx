import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { CommandAnswer } from './CommandAnswer';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

describe('CommandAnswer', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('shows nothing without an answer', () => {
    render(<CommandAnswer text={null} pending={false} onDismiss={vi.fn()} />);
    expect(screen.queryByTestId('command-response')).toBeNull();
  });

  it('shows the answer with a button that dismisses it', () => {
    const onDismiss = vi.fn();
    render(<CommandAnswer text="You are away" pending={false} onDismiss={onDismiss} />);
    expect(screen.getByRole('status')).toHaveTextContent('You are away');
    const dismiss = screen.getByRole('button', { name: 'Dismiss' });
    expect(dismiss).toHaveAttribute('data-tooltip', 'Dismiss');
    fireEvent.click(dismiss);
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('names the button in Japanese', async () => {
    await i18n.changeLanguage('ja');
    render(<CommandAnswer text="You are away" pending={false} onDismiss={vi.fn()} />);
    expect(screen.getByRole('button', { name: '閉じる' })).toHaveAttribute('data-tooltip', '閉じる');
  });

  it('shows an answer as text', () => {
    render(<CommandAnswer text={hostile} pending={false} onDismiss={vi.fn()} />);
    expect(screen.getByRole('status').textContent).toBe(`${hostile}×`);
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('shows the button’s name as text', () => {
    i18n.addResource('en', 'messages', 'commandAnswer.dismiss', hostile);
    try {
      render(<CommandAnswer text="You are away" pending={false} onDismiss={vi.fn()} />);
      expect(screen.getByRole('button', { name: hostile })).toHaveAttribute('data-tooltip', hostile);
      expect(document.querySelector('[onerror]')).toBeNull();
    } finally {
      i18n.addResource('en', 'messages', 'commandAnswer.dismiss', 'Dismiss');
    }
  });

  it('has Japanese for every one', () => {
    const english = Object.keys(i18n.getResource('en', 'messages', 'commandAnswer'));
    const japanese = Object.keys(i18n.getResource('ja', 'messages', 'commandAnswer'));
    expect(english.length).toBeGreaterThan(0);
    expect(japanese).toEqual(english);
  });
});
