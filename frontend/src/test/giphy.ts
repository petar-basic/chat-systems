import type { GifSettings } from '@/features/gifs';

export const SETTINGS: GifSettings = { provider: 'giphy', api_key: 'giphyTestKey123', rating: 'pg' };

export function giphyItem(id: string, title = `Cat ${id}`) {
  return {
    id,
    title,
    alt_text: '',
    images: {
      original: {
        url: `https://media1.giphy.com/media/${id}/giphy.gif?cid=abc&rid=giphy.gif`,
        mp4: `https://media1.giphy.com/media/${id}/giphy.mp4?cid=abc&rid=giphy.mp4`,
        width: '480',
        height: '270',
      },
      original_still: { url: `https://media1.giphy.com/media/${id}/giphy_s.gif?cid=abc` },
      fixed_width: {
        url: `https://media1.giphy.com/media/${id}/200w.gif?cid=abc`,
        webp: `https://media1.giphy.com/media/${id}/200w.webp?cid=abc`,
        width: '200',
        height: '113',
      },
      fixed_width_still: { url: `https://media1.giphy.com/media/${id}/200w_s.gif?cid=abc` },
    },
  };
}

export function giphyPage(ids: string[], total = ids.length, offset = 0) {
  return {
    data: ids.map((id) => giphyItem(id)),
    pagination: { total_count: total, count: ids.length, offset },
  };
}

export function answer(body: unknown, status = 200) {
  return Promise.resolve(
    new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } }),
  );
}
