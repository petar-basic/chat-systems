import type { components } from '@/api/schema';

type VideoEmbed = components['schemas']['VideoEmbed'];

const YOUTUBE_ID = /^[A-Za-z0-9_-]{11}$/;
const VIMEO_ID = /^\d{1,12}$/;
const VIMEO_HASH = /^[0-9a-f]{6,20}$/i;
const LOOM_ID = /^[0-9a-f]{32}$/;

export function embedSrc(embed: VideoEmbed): string | null {
  switch (embed.provider) {
    case 'youtube': {
      if (!YOUTUBE_ID.test(embed.id)) return null;
      const params = new URLSearchParams({ autoplay: '1' });
      if (embed.start && embed.start > 0) params.set('start', String(Math.floor(embed.start)));
      return `https://www.youtube-nocookie.com/embed/${embed.id}?${params}`;
    }
    case 'vimeo': {
      if (!VIMEO_ID.test(embed.id)) return null;
      const params = new URLSearchParams({ autoplay: '1' });
      if (embed.hash) {
        if (!VIMEO_HASH.test(embed.hash)) return null;
        params.set('h', embed.hash);
      }
      return `https://player.vimeo.com/video/${embed.id}?${params}`;
    }
    case 'loom':
      return LOOM_ID.test(embed.id) ? `https://www.loom.com/embed/${embed.id}?autoplay=1` : null;
    default:
      return null;
  }
}
