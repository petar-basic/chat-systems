import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { CachedImage } from './CachedImage';

describe('CachedImage', () => {
  it('waits for the viewport the first time and paints at once after it has loaded', () => {
    const onLoad = vi.fn();
    const first = render(<CachedImage src="/files/cat.png" alt="cat" onLoad={onLoad} />);
    const image = screen.getByRole('img', { name: 'cat' });
    expect(image).toHaveAttribute('loading', 'lazy');
    expect(image).toHaveAttribute('decoding', 'async');

    fireEvent.load(image);
    expect(onLoad).toHaveBeenCalledOnce();
    first.unmount();

    render(<CachedImage src="/files/cat.png" alt="cat" />);
    const again = screen.getByRole('img', { name: 'cat' });
    expect(again).toHaveAttribute('loading', 'eager');
    expect(again).toHaveAttribute('decoding', 'sync');
  });

  it('keeps an image that never loaded lazy', () => {
    const first = render(<CachedImage src="/files/dog.png" alt="dog" />);
    fireEvent.error(screen.getByRole('img', { name: 'dog' }));
    first.unmount();

    render(<CachedImage src="/files/dog.png" alt="dog" />);
    expect(screen.getByRole('img', { name: 'dog' })).toHaveAttribute('loading', 'lazy');
  });
});
