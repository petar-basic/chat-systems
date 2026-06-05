interface MartEmoji {
  id: string;
  name: string;
  skins: { native: string }[];
}

let initPromise: Promise<typeof import('emoji-mart')> | null = null;

function ensureInit() {
  if (!initPromise) {
    initPromise = (async () => {
      const mart = await import('emoji-mart');
      const data = (await import('@emoji-mart/data')).default;
      await mart.init({ data });
      return mart;
    })();
  }
  return initPromise;
}

export interface EmojiSuggestionItem {
  id: string;
  native: string;
  name: string;
}

export async function searchEmojis(query: string, limit = 8): Promise<EmojiSuggestionItem[]> {
  const mart = await ensureInit();
  const results = ((await mart.SearchIndex.search(query)) ?? []) as MartEmoji[];
  return results
    .slice(0, limit)
    .map((e) => ({ id: e.id, native: e.skins?.[0]?.native ?? '', name: e.name }))
    .filter((e) => e.native);
}
