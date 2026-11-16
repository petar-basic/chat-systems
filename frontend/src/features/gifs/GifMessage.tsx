import { useEffect, useRef, useState, type RefObject } from 'react';
import { useTranslation } from 'react-i18next';
import { useReducedMotion } from './useReducedMotion';
import type { Gif } from './types';

const MAX_WIDTH = 360;
const MAX_HEIGHT = 280;

function fitGif(width: number, height: number): { width: number; height: number } {
  const scale = Math.min(1, MAX_WIDTH / width, MAX_HEIGHT / height);
  return { width: Math.max(1, Math.round(width * scale)), height: Math.max(1, Math.round(height * scale)) };
}

function useOnScreen(ref: RefObject<HTMLElement | null>): boolean {
  const [onScreen, setOnScreen] = useState(() => typeof IntersectionObserver === 'undefined');
  useEffect(() => {
    const node = ref.current;
    if (!node || typeof IntersectionObserver === 'undefined') return undefined;
    const observer = new IntersectionObserver(([entry]) => setOnScreen(!!entry?.isIntersecting));
    observer.observe(node);
    return () => observer.disconnect();
  }, [ref]);
  return onScreen;
}

export function GifMessage({ gif }: { gif: Gif }) {
  const { t } = useTranslation('messages');
  const video = useRef<HTMLVideoElement>(null);
  const reduced = useReducedMotion();
  const onScreen = useOnScreen(video);
  const [held, setHeld] = useState(false);
  const playing = onScreen && (!reduced || held);
  const size = fitGif(gif.width, gif.height);

  useEffect(() => {
    const node = video.current;
    if (!node) return;
    if (playing) void Promise.resolve(node.play()).catch(() => undefined);
    else node.pause();
  }, [playing]);

  return (
    <figure className="mt-1 w-fit" data-qa="message-gif">
      <video
        ref={video}
        src={gif.url}
        poster={gif.still}
        width={size.width}
        height={size.height}
        style={{ aspectRatio: `${size.width} / ${size.height}` }}
        muted
        loop
        playsInline
        preload={reduced ? 'none' : 'metadata'}
        tabIndex={0}
        aria-label={gif.title || t('gifs.untitled')}
        data-playing={playing}
        onMouseEnter={() => setHeld(true)}
        onMouseLeave={() => setHeld(false)}
        onFocus={() => setHeld(true)}
        onBlur={() => setHeld(false)}
        className="block max-w-full h-auto object-contain rounded-lg bg-raised/40 focus-visible:outline-2 focus-visible:outline-accent"
      />
      <figcaption className="mt-0.5 text-2xs text-subtle">{t('gifs.via')}</figcaption>
    </figure>
  );
}
