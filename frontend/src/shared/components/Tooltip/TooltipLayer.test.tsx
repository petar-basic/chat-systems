import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { TooltipLayer } from './TooltipLayer';

function renderLayer() {
  return render(
    <>
      <TooltipLayer />
      <button aria-label="Reply in thread" data-tooltip="Reply in thread">
        <svg data-qa="icon" />
      </button>
      <button aria-label="Add reaction" data-tooltip="Add reaction" />
      <button data-tooltip="Message Bob">Bob</button>
      <span data-hover-card="">
        <img alt=":shipit:" data-tooltip=":shipit:" />
      </span>
      <p>Plain text</p>
    </>,
  );
}

const hover = (el: Element, pointerType = 'mouse') => fireEvent.pointerOver(el, { pointerType });
const tooltip = () => screen.queryByRole('tooltip', { hidden: true });

describe('TooltipLayer', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('waits before the first tooltip and then names the button', () => {
    renderLayer();
    hover(screen.getByTestId('icon'));
    act(() => vi.advanceTimersByTime(300));
    expect(tooltip()).toBeNull();
    act(() => vi.advanceTimersByTime(150));
    expect(tooltip()).toHaveTextContent('Reply in thread');
  });

  it('moves to the next button at once while one is showing', () => {
    renderLayer();
    hover(screen.getByRole('button', { name: 'Reply in thread' }));
    act(() => vi.advanceTimersByTime(400));
    hover(screen.getByRole('button', { name: 'Add reaction' }));
    expect(tooltip()).toHaveTextContent('Add reaction');
  });

  it('hides when the pointer leaves for plain content, and again on a press', () => {
    renderLayer();
    const reply = screen.getByRole('button', { name: 'Reply in thread' });
    hover(reply);
    act(() => vi.advanceTimersByTime(400));
    hover(screen.getByText('Plain text'));
    expect(tooltip()).toBeNull();

    hover(reply);
    expect(tooltip()).toHaveTextContent('Reply in thread');
    fireEvent.pointerDown(reply);
    expect(tooltip()).toBeNull();
    hover(screen.getByTestId('icon'));
    act(() => vi.advanceTimersByTime(400));
    expect(tooltip()).toBeNull();
  });

  it('hides on Escape', () => {
    renderLayer();
    hover(screen.getByRole('button', { name: 'Add reaction' }));
    act(() => vi.advanceTimersByTime(400));
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(tooltip()).toBeNull();
  });

  it('never shows on touch', () => {
    renderLayer();
    hover(screen.getByRole('button', { name: 'Add reaction' }), 'touch');
    act(() => vi.advanceTimersByTime(1000));
    expect(tooltip()).toBeNull();
  });

  it('leaves a hover card trigger to its own card', () => {
    renderLayer();
    hover(screen.getByAltText(':shipit:'));
    act(() => vi.advanceTimersByTime(1000));
    expect(tooltip()).toBeNull();
  });

  it('describes its element only when the text is not already its name', () => {
    renderLayer();
    const bob = screen.getByRole('button', { name: 'Bob' });
    hover(bob);
    act(() => vi.advanceTimersByTime(400));
    expect(bob).toHaveAttribute('aria-describedby', tooltip()!.id);

    const reply = screen.getByRole('button', { name: 'Reply in thread' });
    hover(reply);
    expect(reply).not.toHaveAttribute('aria-describedby');
    expect(bob).not.toHaveAttribute('aria-describedby');
  });

  it('follows a tooltip that changes while it shows', async () => {
    renderLayer();
    const bob = screen.getByRole('button', { name: 'Bob' });
    hover(bob);
    act(() => vi.advanceTimersByTime(400));
    await act(async () => {
      bob.setAttribute('data-tooltip', 'Message Bob — on holiday');
    });
    expect(tooltip()).toHaveTextContent('Message Bob — on holiday');
  });
});
