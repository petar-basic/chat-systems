import type { GifRating } from './types';

export const GIF_RATINGS: GifRating[] = ['g', 'pg', 'pg-13', 'r'];

function settingOf(settings: unknown, key: string): unknown {
  if (typeof settings !== 'object' || settings === null) return undefined;
  return (settings as Record<string, unknown>)[key];
}

export function gifsEnabled(settings: unknown): boolean {
  return settingOf(settings, 'gifs') !== false;
}

export function gifRatingOf(settings: unknown): GifRating {
  const rating = settingOf(settings, 'gif_rating');
  return GIF_RATINGS.find((r) => r === rating) ?? 'pg';
}
