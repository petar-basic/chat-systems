import type { components } from '@/api/schema';

export type Gif = components['schemas']['Gif'];
export type GifSettings = components['schemas']['GifSettings'];
export type GifRating = components['schemas']['GifRating'];

export interface GifChoice {
  gif: Gif;
  preview: string;
  still: string;
  width: number;
  height: number;
}

export interface GifPage {
  choices: GifChoice[];
  next: number | undefined;
}
