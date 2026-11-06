import type { ImgHTMLAttributes } from 'react';

const KEEP = 200;
const kept = new Map<string, HTMLImageElement>();

function keep(src: string) {
  const held = kept.get(src);
  kept.delete(src);
  if (held) {
    kept.set(src, held);
    return;
  }
  const image = new Image();
  image.referrerPolicy = 'no-referrer';
  image.src = src;
  kept.set(src, image);
  if (kept.size > KEEP) {
    const oldest = kept.keys().next().value;
    if (oldest !== undefined) kept.delete(oldest);
  }
}

type Props = Omit<ImgHTMLAttributes<HTMLImageElement>, 'src' | 'alt' | 'loading' | 'decoding'> & {
  src: string;
  alt: string;
};

export function CachedImage({ src, alt, onLoad, ...rest }: Props) {
  const seen = kept.has(src);
  return (
    <img
      {...rest}
      src={src}
      alt={alt}
      loading={seen ? 'eager' : 'lazy'}
      decoding={seen ? 'sync' : 'async'}
      onLoad={(event) => {
        keep(src);
        onLoad?.(event);
      }}
    />
  );
}
