import { describe, it, expect } from 'vitest';
import type { InfiniteData } from '@tanstack/react-query';
import { hasMessage, upsertMessage, patchMessageById, removeMessageById, newestFirst } from './messageCache';

interface Msg {
  id: string;
  content: string;
  pending?: boolean;
  created_at: string;
}
type Page = { data: Msg[]; next_cursor?: string | null };
type Cache = InfiniteData<Page>;

const m = (id: string, content = id, extra: Partial<Msg> = {}): Msg => ({
  id,
  content,
  created_at: '2026-01-01T00:00:00.000Z',
  ...extra,
});

function cache(pages: Msg[][], cursors: (string | null)[] = []): Cache {
  return {
    pages: pages.map((data, i) => ({ data, next_cursor: cursors[i] ?? null })),
    pageParams: pages.map(() => undefined),
  };
}

describe('messageCache.hasMessage', () => {
  it('finds a message across pages', () => {
    const c = cache([[m('a'), m('b')], [m('c')]]);
    expect(hasMessage(c, 'c')).toBe(true);
    expect(hasMessage(c, 'z')).toBe(false);
  });
  it('is false for an undefined cache', () => {
    expect(hasMessage(undefined, 'a')).toBe(false);
  });
});

describe('messageCache.upsertMessage', () => {
  it('inserts a new message at the head of the last page', () => {
    const c = cache([[m('a')], [m('b')]]);
    const next = upsertMessage(c, m('new'), 'lastPage')!;
    expect(next.pages[0].data.map((x) => x.id)).toEqual(['a']);
    expect(next.pages[1].data.map((x) => x.id)).toEqual(['new', 'b']);
  });

  it('inserts a new message at the head of the first page', () => {
    const c = cache([[m('a')], [m('b')]]);
    const next = upsertMessage(c, m('new'), 'firstPage')!;
    expect(next.pages[0].data.map((x) => x.id)).toEqual(['new', 'a']);
    expect(next.pages[1].data.map((x) => x.id)).toEqual(['b']);
  });

  it('dedups by id — replaces in place instead of inserting a duplicate', () => {
    const c = cache([[m('a', 'old', { pending: true })]]);
    const next = upsertMessage(c, m('a', 'confirmed'), 'lastPage')!;
    expect(next.pages[0].data).toHaveLength(1);
    expect(next.pages[0].data[0].content).toBe('confirmed');
    expect(next.pages[0].data[0].pending).toBeUndefined();
  });

  it('preserves extra page fields (e.g. next_cursor) on insert', () => {
    const c = cache([[m('a')]], ['cursor-1']);
    const next = upsertMessage(c, m('new'), 'lastPage')!;
    expect(next.pages[0].next_cursor).toBe('cursor-1');
  });

  it('returns the cache unchanged when undefined or empty', () => {
    expect(upsertMessage(undefined, m('x'), 'lastPage')).toBeUndefined();
    const empty = cache([]);
    expect(upsertMessage(empty, m('x'), 'lastPage')).toBe(empty);
  });

  it('keeps the target page ordered by created_at when an out-of-order message arrives', () => {
    const t = (s: string) => `2026-06-05T10:00:0${s}.000Z`;
    const c = cache([[m('c', 'c', { created_at: t('3') }), m('b', 'b', { created_at: t('2') })]]);
    const late = m('a', 'a', { created_at: t('1') });
    const next = upsertMessage(c, late, 'firstPage', newestFirst)!;
    expect(next.pages[0].data.map((x) => x.id)).toEqual(['c', 'b', 'a']);
  });

  it('places a newer realtime message ahead of older ones regardless of insert edge', () => {
    const t = (s: string) => `2026-06-05T10:00:0${s}.000Z`;
    const c = cache([[m('b', 'b', { created_at: t('2') }), m('a', 'a', { created_at: t('1') })]]);
    const newer = m('c', 'c', { created_at: t('3') });
    const next = upsertMessage(c, newer, 'lastPage', newestFirst)!;
    expect(next.pages[0].data.map((x) => x.id)).toEqual(['c', 'b', 'a']);
  });
});

describe('messageCache.patchMessageById', () => {
  it('patches only the matching message', () => {
    const c = cache([[m('a'), m('b')]]);
    const next = patchMessageById(c, 'b', (msg) => ({ ...msg, content: 'edited' }))!;
    expect(next.pages[0].data[0].content).toBe('a');
    expect(next.pages[0].data[1].content).toBe('edited');
  });
  it('is a no-op when the id is absent', () => {
    const c = cache([[m('a')]]);
    const next = patchMessageById(c, 'zzz', (msg) => ({ ...msg, content: 'x' }))!;
    expect(next.pages[0].data[0].content).toBe('a');
  });
});

describe('messageCache.removeMessageById', () => {
  it('removes the matching message and leaves the rest', () => {
    const c = cache([[m('a'), m('b')], [m('c')]]);
    const next = removeMessageById(c, 'b')!;
    expect(next.pages[0].data.map((x) => x.id)).toEqual(['a']);
    expect(next.pages[1].data.map((x) => x.id)).toEqual(['c']);
  });
});
