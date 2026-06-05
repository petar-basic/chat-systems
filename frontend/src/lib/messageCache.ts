import type { InfiniteData } from '@tanstack/react-query';

interface WithId {
  id: string;
}

type Page = { data: WithId[] };
type ItemOf<P extends Page> = P['data'][number];

export type InsertEdge = 'firstPage' | 'lastPage';

export const newestFirst = <T extends { created_at: string }>(a: T, b: T): number =>
  b.created_at.localeCompare(a.created_at);

function rebuild<P extends Page>(page: P, data: ItemOf<P>[]): P {
  return { ...page, data } as P;
}

export function hasMessage<P extends Page>(cache: InfiniteData<P> | undefined, id: string): boolean {
  return !!cache?.pages.some((p) => p.data.some((m) => m.id === id));
}

export function upsertMessage<P extends Page>(
  cache: InfiniteData<P> | undefined,
  message: ItemOf<P>,
  edge: InsertEdge,
  compare?: (a: ItemOf<P>, b: ItemOf<P>) => number,
): InfiniteData<P> | undefined {
  if (!cache) return cache;

  if (hasMessage(cache, message.id)) {
    return {
      ...cache,
      pages: cache.pages.map((p) =>
        p.data.some((m) => m.id === message.id)
          ? rebuild(
              p,
              p.data.map((m) => (m.id === message.id ? message : m)),
            )
          : p,
      ),
    };
  }

  if (cache.pages.length === 0) return cache;
  const targetIndex = edge === 'lastPage' ? cache.pages.length - 1 : 0;
  return {
    ...cache,
    pages: cache.pages.map((p, i) => {
      if (i !== targetIndex) return p;
      const data = [message, ...p.data];
      return rebuild(p, compare ? data.sort(compare) : data);
    }),
  };
}

export function patchMessageById<P extends Page>(
  cache: InfiniteData<P> | undefined,
  id: string,
  updater: (message: ItemOf<P>) => ItemOf<P>,
): InfiniteData<P> | undefined {
  if (!cache) return cache;
  return {
    ...cache,
    pages: cache.pages.map((p) =>
      p.data.some((m) => m.id === id)
        ? rebuild(
            p,
            p.data.map((m) => (m.id === id ? updater(m) : m)),
          )
        : p,
    ),
  };
}

export function removeMessageById<P extends Page>(
  cache: InfiniteData<P> | undefined,
  id: string,
): InfiniteData<P> | undefined {
  if (!cache) return cache;
  return {
    ...cache,
    pages: cache.pages.map((p) =>
      p.data.some((m) => m.id === id)
        ? rebuild(
            p,
            p.data.filter((m) => m.id !== id),
          )
        : p,
    ),
  };
}
