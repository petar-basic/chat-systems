import type { GifChoice, GifPage, GifSettings } from './types';

export const GIF_PAGE = 24;
const API = 'https://api.giphy.com/v1/gifs';
const TITLE_MAX = 200;

interface Rendition {
  url?: string;
  width?: string;
  height?: string;
  mp4?: string;
  webp?: string;
}

interface GiphyItem {
  id: string;
  title?: string;
  alt_text?: string;
  images?: Record<string, Rendition | undefined>;
}

interface GiphyPage {
  data?: GiphyItem[];
  pagination?: { total_count?: number; count?: number; offset?: number };
}

function sizeOf(rendition: Rendition | undefined): { width: number; height: number } | null {
  const width = Number(rendition?.width);
  const height = Number(rendition?.height);
  return width > 0 && height > 0 ? { width, height } : null;
}

export function choiceOf(item: GiphyItem): GifChoice | null {
  const original = item.images?.original;
  const grid = item.images?.fixed_width;
  const full = sizeOf(original);
  const small = sizeOf(grid);
  const still = item.images?.original_still?.url;
  const preview = grid?.webp ?? grid?.url;
  if (!original?.mp4 || !still || !full || !small || !preview) return null;
  return {
    gif: {
      provider: 'giphy',
      id: item.id,
      title: (item.alt_text || item.title || '').trim().slice(0, TITLE_MAX),
      url: original.mp4,
      still,
      width: full.width,
      height: full.height,
    },
    preview,
    still: item.images?.fixed_width_still?.url ?? still,
    width: small.width,
    height: small.height,
  };
}

export async function findGifs(
  settings: GifSettings,
  query: string,
  offset: number,
  signal?: AbortSignal,
): Promise<GifPage> {
  const words = query.trim();
  const params = new URLSearchParams({
    api_key: settings.api_key,
    limit: String(GIF_PAGE),
    offset: String(offset),
    rating: settings.rating,
  });
  if (words) params.set('q', words);
  const res = await fetch(`${API}/${words ? 'search' : 'trending'}?${params}`, { signal });
  if (!res.ok) throw new Error(`GIPHY answered ${res.status}`);
  const page = (await res.json()) as GiphyPage;
  const items = page.data ?? [];
  const seen = (page.pagination?.offset ?? offset) + items.length;
  const total = page.pagination?.total_count ?? seen;
  return {
    choices: items.flatMap((item) => choiceOf(item) ?? []),
    next: items.length === GIF_PAGE && seen < total ? seen : undefined,
  };
}

export function parseGiphyCommand(text: string): string | null {
  const match = /^\/giphy(?:\s+([\s\S]*))?$/i.exec(text.trim());
  return match ? (match[1] ?? '').trim() : null;
}
