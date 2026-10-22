import { describe, it, expect } from 'vitest';
import { embedSrc } from './videoEmbed';

describe('embedSrc', () => {
  it('builds each player from the video id', () => {
    expect(embedSrc({ provider: 'youtube', id: 'dQw4w9WgXcQ', start: 90 })).toBe(
      'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?autoplay=1&start=90',
    );
    expect(embedSrc({ provider: 'vimeo', id: '76979871', hash: 'a1b2c3d4e5' })).toBe(
      'https://player.vimeo.com/video/76979871?autoplay=1&h=a1b2c3d4e5',
    );
    expect(embedSrc({ provider: 'loom', id: '0123456789abcdef0123456789abcdef' })).toBe(
      'https://www.loom.com/embed/0123456789abcdef0123456789abcdef?autoplay=1',
    );
  });

  it('refuses anything that is not a plain id', () => {
    expect(embedSrc({ provider: 'youtube', id: '../../evil' })).toBeNull();
    expect(embedSrc({ provider: 'youtube', id: 'dQw4w9WgXcQ?x=' })).toBeNull();
    expect(embedSrc({ provider: 'vimeo', id: '1', hash: 'x"onload' })).toBeNull();
    expect(embedSrc({ provider: 'loom', id: 'abc' })).toBeNull();
    expect(embedSrc({ provider: 'evil', id: 'dQw4w9WgXcQ' })).toBeNull();
  });
});
