import type { GifRating } from './types';

export const GIF_RATINGS: { rating: GifRating; label: string }[] = [
  { rating: 'g', label: 'G — fit for everyone' },
  { rating: 'pg', label: 'PG — mild' },
  { rating: 'pg-13', label: 'PG-13 — some adult themes' },
  { rating: 'r', label: 'R — adult' },
];

function settingOf(settings: unknown, key: string): unknown {
  if (typeof settings !== 'object' || settings === null) return undefined;
  return (settings as Record<string, unknown>)[key];
}

export function gifsEnabled(settings: unknown): boolean {
  return settingOf(settings, 'gifs') !== false;
}

export function gifRatingOf(settings: unknown): GifRating {
  const rating = settingOf(settings, 'gif_rating');
  return GIF_RATINGS.find((r) => r.rating === rating)?.rating ?? 'pg';
}
