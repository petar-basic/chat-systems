import { describe, it, expect, vi, afterEach } from 'vitest';
import { railIcon } from './railIcon';

describe('railIcon', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('draws only icons from this server', async () => {
    const fetch = vi.fn();
    vi.stubGlobal('fetch', fetch);

    expect(await railIcon('https://tracker.example.net/pixel.png')).toBeNull();
    expect(fetch).not.toHaveBeenCalled();
  });

  it('gives up on an icon the server refuses, and tries it again next time', async () => {
    const fetch = vi.fn().mockResolvedValue(new Response(null, { status: 401 }));
    vi.stubGlobal('fetch', fetch);

    expect(await railIcon('/api/files/download/icons/a1.png')).toBeNull();
    expect(await railIcon('/api/files/download/icons/a1.png')).toBeNull();
    expect(fetch).toHaveBeenCalledTimes(2);
  });
});
