import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { GifMessage } from './GifMessage';
import type { Gif } from './types';

const GIF: Gif = {
  provider: 'giphy',
  id: 'abc123',
  title: 'Happy cat',
  url: 'https://media1.giphy.com/media/abc123/giphy.mp4?cid=abc',
  still: 'https://media1.giphy.com/media/abc123/giphy_s.gif?cid=abc',
  width: 720,
  height: 405,
};

const play = vi.fn(() => Promise.resolve());
const pause = vi.fn();
let seen: ((entries: { isIntersecting: boolean }[]) => void) | undefined;

function reduceMotion() {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: query === '(prefers-reduced-motion: reduce)',
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
}

function watchScreen() {
  vi.stubGlobal(
    'IntersectionObserver',
    class {
      constructor(callback: (entries: { isIntersecting: boolean }[]) => void) {
        seen = callback;
      }
      observe() {}
      disconnect() {}
    },
  );
}

beforeEach(() => {
  play.mockClear();
  pause.mockClear();
  seen = undefined;
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(play);
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(pause);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('GifMessage', () => {
  it('plays the MP4 muted and looping over its still, scaled to fit', () => {
    render(<GifMessage gif={GIF} />);
    const video = screen.getByLabelText('Happy cat') as HTMLVideoElement;
    expect(video).toHaveAttribute('src', GIF.url);
    expect(video).toHaveAttribute('poster', GIF.still);
    expect(video.muted).toBe(true);
    expect(video.loop).toBe(true);
    expect(video).toHaveAttribute('width', '360');
    expect(video).toHaveAttribute('height', '203');
    expect(video).toHaveAttribute('data-playing', 'true');
    expect(play).toHaveBeenCalled();
    expect(screen.getByText('via GIPHY')).toBeInTheDocument();
  });

  it('calls an untitled GIF a GIF', () => {
    render(<GifMessage gif={{ ...GIF, title: '' }} />);
    expect(screen.getByLabelText('GIF')).toBeInTheDocument();
  });

  it('holds still under reduced motion until hovered or focused', () => {
    reduceMotion();
    render(<GifMessage gif={GIF} />);
    const video = screen.getByLabelText('Happy cat');
    expect(video).toHaveAttribute('data-playing', 'false');
    expect(video).toHaveAttribute('preload', 'none');
    expect(play).not.toHaveBeenCalled();

    fireEvent.mouseEnter(video);
    expect(video).toHaveAttribute('data-playing', 'true');
    expect(play).toHaveBeenCalledOnce();

    fireEvent.mouseLeave(video);
    expect(video).toHaveAttribute('data-playing', 'false');
    expect(pause).toHaveBeenCalled();

    fireEvent.focus(video);
    expect(video).toHaveAttribute('data-playing', 'true');
    fireEvent.blur(video);
    expect(video).toHaveAttribute('data-playing', 'false');
  });

  it('plays only while it is on screen', () => {
    watchScreen();
    render(<GifMessage gif={GIF} />);
    const video = screen.getByLabelText('Happy cat');
    expect(video).toHaveAttribute('data-playing', 'false');
    expect(play).not.toHaveBeenCalled();

    act(() => seen?.([{ isIntersecting: true }]));
    expect(video).toHaveAttribute('data-playing', 'true');
    expect(play).toHaveBeenCalledOnce();

    act(() => seen?.([{ isIntersecting: false }]));
    expect(video).toHaveAttribute('data-playing', 'false');
    expect(pause).toHaveBeenCalled();
  });
});
