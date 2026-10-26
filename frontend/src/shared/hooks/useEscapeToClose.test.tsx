import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { useEscapeToClose } from './useEscapeToClose';

function Closer({ onClose, active = true }: { onClose: () => void; active?: boolean }) {
  useEscapeToClose(onClose, active);
  return null;
}

describe('useEscapeToClose', () => {
  it('closes only what was opened last', () => {
    const outer = vi.fn();
    const inner = vi.fn();
    const { rerender } = render(<Closer onClose={outer} />);
    rerender(
      <>
        <Closer onClose={outer} />
        <Closer onClose={inner} />
      </>,
    );
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(inner).toHaveBeenCalledOnce();
    expect(outer).not.toHaveBeenCalled();
  });

  it('hands Escape back once the inner one closes', () => {
    const outer = vi.fn();
    const { rerender } = render(
      <>
        <Closer onClose={outer} />
        <Closer onClose={vi.fn()} />
      </>,
    );
    rerender(<Closer onClose={outer} />);
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(outer).toHaveBeenCalledOnce();
  });

  it('ignores an inactive handler and other keys', () => {
    const outer = vi.fn();
    const idle = vi.fn();
    render(
      <>
        <Closer onClose={outer} />
        <Closer onClose={idle} active={false} />
      </>,
    );
    fireEvent.keyDown(document, { key: 'Enter' });
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(idle).not.toHaveBeenCalled();
    expect(outer).toHaveBeenCalledOnce();
  });

  it('calls the latest callback without re-registering', () => {
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = render(<Closer onClose={first} />);
    rerender(<Closer onClose={second} />);
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledOnce();
  });
});
