import { afterEach, describe, expect, it, vi } from 'vitest';
import { choiceOf, findGifs, GIF_PAGE, parseGiphyCommand } from './giphy';
import { SETTINGS, answer, giphyItem, giphyPage } from '@/test/giphy';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('choiceOf', () => {
  it('sends the original MP4 and shows the small WebP in the grid', () => {
    const choice = choiceOf(giphyItem('abc123', '  Happy cat  '));
    expect(choice).toEqual({
      gif: {
        provider: 'giphy',
        id: 'abc123',
        title: 'Happy cat',
        url: 'https://media1.giphy.com/media/abc123/giphy.mp4?cid=abc&rid=giphy.mp4',
        still: 'https://media1.giphy.com/media/abc123/giphy_s.gif?cid=abc',
        width: 480,
        height: 270,
      },
      preview: 'https://media1.giphy.com/media/abc123/200w.webp?cid=abc',
      still: 'https://media1.giphy.com/media/abc123/200w_s.gif?cid=abc',
      width: 200,
      height: 113,
    });
  });

  it('names the GIF by its alt text before its title', () => {
    const item = { ...giphyItem('abc'), alt_text: 'A cat waving' };
    expect(choiceOf(item)?.gif.title).toBe('A cat waving');
  });

  it('keeps a long title to 200 characters', () => {
    expect(choiceOf(giphyItem('abc', 'x'.repeat(300)))?.gif.title).toHaveLength(200);
  });

  it('falls back to the GIF file when the grid has no WebP', () => {
    const item = giphyItem('abc');
    const grid = { url: item.images.fixed_width.url, width: '200', height: '113' };
    expect(choiceOf({ ...item, images: { ...item.images, fixed_width: grid } })?.preview).toBe(grid.url);
  });

  it('leaves out a GIF without an MP4 or a size', () => {
    const item = giphyItem('abc');
    const original = { url: item.images.original.url, width: '480', height: '270' };
    expect(choiceOf({ ...item, images: { ...item.images, original } })).toBeNull();
    expect(
      choiceOf({ ...item, images: { ...item.images, original: { ...item.images.original, width: '0' } } }),
    ).toBeNull();
  });
});

describe('findGifs', () => {
  it('asks GIPHY for trending GIFs at the workspace rating when nothing is typed', async () => {
    const fetch = vi.fn<(url: string) => Promise<Response>>(() => answer(giphyPage(['a', 'b'])));
    vi.stubGlobal('fetch', fetch);

    const page = await findGifs(SETTINGS, '  ', 0);

    const url = new URL(fetch.mock.calls[0]?.[0] ?? '');
    expect(url.origin + url.pathname).toBe('https://api.giphy.com/v1/gifs/trending');
    expect(Object.fromEntries(url.searchParams)).toEqual({
      api_key: SETTINGS.api_key,
      limit: String(GIF_PAGE),
      offset: '0',
      rating: 'pg',
    });
    expect(page.choices.map((c) => c.gif.id)).toEqual(['a', 'b']);
    expect(page.next).toBeUndefined();
  });

  it('searches the typed words and offers the next page while GIPHY has more', async () => {
    const ids = Array.from({ length: GIF_PAGE }, (_, i) => `g${i}`);
    const fetch = vi.fn<(url: string) => Promise<Response>>(() => answer(giphyPage(ids, 100, 24)));
    vi.stubGlobal('fetch', fetch);

    const page = await findGifs({ ...SETTINGS, rating: 'g' }, ' happy cat ', 24);

    const url = new URL(fetch.mock.calls[0]?.[0] ?? '');
    expect(url.pathname).toBe('/v1/gifs/search');
    expect(url.searchParams.get('q')).toBe('happy cat');
    expect(url.searchParams.get('offset')).toBe('24');
    expect(url.searchParams.get('rating')).toBe('g');
    expect(page.next).toBe(48);
  });

  it('fails when GIPHY answers with an error', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn<(url: string) => Promise<Response>>(() => answer({ message: 'Invalid key' }, 401)),
    );
    await expect(findGifs(SETTINGS, 'cat', 0)).rejects.toThrow('GIPHY answered 401');
  });
});

describe('parseGiphyCommand', () => {
  it('reads the words after /giphy', () => {
    expect(parseGiphyCommand('/giphy  happy cat ')).toBe('happy cat');
    expect(parseGiphyCommand('/GIPHY dance')).toBe('dance');
  });

  it('opens the picker empty for /giphy alone', () => {
    expect(parseGiphyCommand('/giphy')).toBe('');
  });

  it('leaves other text alone', () => {
    expect(parseGiphyCommand('/giphycat')).toBeNull();
    expect(parseGiphyCommand('send a /giphy')).toBeNull();
    expect(parseGiphyCommand('/poll lunch')).toBeNull();
  });
});
