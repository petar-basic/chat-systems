import { create } from 'zustand';
import type { components } from '@/api/schema';
import { logger } from '@/lib/logger';

export type TextSize = components['schemas']['TextSize'];
export type Typeface = components['schemas']['Typeface'];

export const TEXT_SIZES: readonly TextSize[] = ['90', '100', '112', '125'];
export const TYPEFACES: readonly Typeface[] = ['system', 'inter', 'atkinson_hyperlegible', 'open_dyslexic'];

const SIZE_KEY = 'chat_text_size';
const TYPEFACE_KEY = 'chat_typeface';

const FONTS: Record<Exclude<Typeface, 'system'>, () => Promise<unknown>> = {
  inter: () =>
    Promise.all([
      import('@fontsource/inter/400.css'),
      import('@fontsource/inter/500.css'),
      import('@fontsource/inter/600.css'),
      import('@fontsource/inter/700.css'),
    ]),
  atkinson_hyperlegible: () =>
    Promise.all([
      import('@fontsource/atkinson-hyperlegible/400.css'),
      import('@fontsource/atkinson-hyperlegible/700.css'),
    ]),
  open_dyslexic: () =>
    Promise.all([import('@fontsource/opendyslexic/400.css'), import('@fontsource/opendyslexic/700.css')]),
};

function isTextSize(value: unknown): value is TextSize {
  return TEXT_SIZES.includes(value as TextSize);
}

function isTypeface(value: unknown): value is Typeface {
  return TYPEFACES.includes(value as Typeface);
}

function read<T>(key: string, accept: (value: unknown) => value is T, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return accept(raw) ? raw : fallback;
  } catch {
    return fallback;
  }
}

function keep(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    return;
  }
}

function applySize(size: TextSize) {
  document.documentElement.style.fontSize = size === '100' ? '' : `${size}%`;
}

function applyTypeface(typeface: Typeface) {
  if (typeface === 'system') delete document.documentElement.dataset.typeface;
  else {
    document.documentElement.dataset.typeface = typeface;
    FONTS[typeface]().catch((err) => logger.error('text', `font ${typeface}`, err));
  }
}

interface TextState {
  size: TextSize;
  typeface: Typeface;
  setSize: (size: TextSize) => void;
  setTypeface: (typeface: Typeface) => void;
}

export const useTextStore = create<TextState>((set, get) => {
  const size = read(SIZE_KEY, isTextSize, '100');
  const typeface = read(TYPEFACE_KEY, isTypeface, 'system');
  applySize(size);
  applyTypeface(typeface);
  return {
    size,
    typeface,
    setSize: (next) => {
      if (next === get().size) return;
      keep(SIZE_KEY, next);
      applySize(next);
      set({ size: next });
    },
    setTypeface: (next) => {
      if (next === get().typeface) return;
      keep(TYPEFACE_KEY, next);
      applyTypeface(next);
      set({ typeface: next });
    },
  };
});
