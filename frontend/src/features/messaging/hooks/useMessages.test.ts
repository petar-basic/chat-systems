import { describe, it, expect } from 'vitest';
import {
  eachPageOldestFirst,
  olderMessagesCursor,
  messagesOldestFirst,
  type MessagesInfiniteData,
  type MessagesResponse,
} from './useMessages';
import { MESSAGES_PAGE_SIZE } from '@/shared/constants';
import { newestFirst, removeMessageById, upsertMessage } from '@/lib/messageCache';
import type { Message } from '@/stores/workspace';

const BASE = Date.parse('2026-01-01T12:00:00.000Z');

function message(id: string, createdAt: number): Message {
  const iso = new Date(createdAt).toISOString();
  return {
    id,
    channel_id: 'ch-1',
    user_id: 'user-1',
    content: `content of ${id}`,
    created_at: iso,
    updated_at: iso,
    deleted_at: null,
    thread_parent_id: null,
    reply_count: 0,
    is_pinned: false,
  } as Message;
}

function serverPage(count: number): MessagesResponse {
  return { data: Array.from({ length: count }, (_, i) => message(`msg-${i}`, BASE - i * 1000)) };
}

function selectedPage(ids: string[], firstMinute: number): MessagesResponse {
  return { data: ids.map((id, i) => message(id, BASE + (firstMinute + i) * 60_000)) };
}

function cache(pages: MessagesResponse[]): MessagesInfiniteData {
  return { pages, pageParams: pages.map(() => undefined) };
}

describe('olderMessagesCursor', () => {
  it('points at the oldest message of the page, not the newest', () => {
    const full = serverPage(MESSAGES_PAGE_SIZE);

    expect(olderMessagesCursor(full)).toBe(`msg-${MESSAGES_PAGE_SIZE - 1}`);
    expect(olderMessagesCursor(full)).not.toBe(full.data[0].id);
  });

  it('stops paging once a page comes back short', () => {
    expect(olderMessagesCursor(serverPage(MESSAGES_PAGE_SIZE - 1))).toBeUndefined();
    expect(olderMessagesCursor(serverPage(0))).toBeUndefined();
  });
});

describe('messagesOldestFirst', () => {
  const newest = selectedPage(['c', 'd'], 2);
  const older = selectedPage(['a', 'b'], 0);

  it('puts the page fetched later — the older one — above the first page', () => {
    const flat = messagesOldestFirst(cache([newest, older]));

    expect(flat.map((m) => m.id)).toEqual(['a', 'b', 'c', 'd']);
  });

  it('keeps every message in ascending created_at order across pages', () => {
    const flat = messagesOldestFirst(cache([newest, older]));
    const timestamps = flat.map((m) => Date.parse(m.created_at));

    expect([...timestamps].sort((x, y) => x - y)).toEqual(timestamps);
  });

  it('returns an empty list before the first page arrives', () => {
    expect(messagesOldestFirst(undefined)).toEqual([]);
  });
});

describe('a message added to a channel that has loaded more than one page', () => {
  const newest = serverPage(MESSAGES_PAGE_SIZE);
  const older: MessagesResponse = {
    data: Array.from({ length: 10 }, (_, i) => message(`old-${i}`, BASE - (MESSAGES_PAGE_SIZE + i) * 1000)),
  };
  const two = cache([newest, older]);
  const shown = (data: MessagesInfiniteData | undefined) =>
    messagesOldestFirst(data && eachPageOldestFirst(data));
  const lastRow = (data: MessagesInfiniteData | undefined) => shown(data).at(-1)?.id;

  it('is the last row when it is sent', () => {
    const sent = { ...message('client-1', BASE + 60_000), pending: true };

    expect(lastRow(upsertMessage(two, sent, newestFirst))).toBe('client-1');
  });

  it('is still the last row once the server confirms it', () => {
    const pending = upsertMessage(two, { ...message('client-1', BASE + 60_000), pending: true }, newestFirst);
    const confirmed = upsertMessage(
      removeMessageById(pending, 'client-1'),
      message('server-1', BASE + 61_000),
      newestFirst,
    );

    expect(lastRow(confirmed)).toBe('server-1');
    expect(shown(confirmed).filter((m) => m.id === 'client-1')).toEqual([]);
  });

  it('is the last row when somebody else sends it', () => {
    const arrived = upsertMessage(two, message('theirs', BASE + 30_000), newestFirst);

    expect(lastRow(arrived)).toBe('theirs');
    expect(arrived?.pages[0].data).toHaveLength(MESSAGES_PAGE_SIZE + 1);
    expect(arrived?.pages[1]).toBe(older);
  });

  it('keeps the history above it in order', () => {
    const arrived = upsertMessage(two, message('theirs', BASE + 30_000), newestFirst);
    const timestamps = shown(arrived).map((m) => Date.parse(m.created_at));

    expect([...timestamps].sort((x, y) => x - y)).toEqual(timestamps);
  });
});
