import { describe, it, expect } from 'vitest';
import {
  olderMessagesCursor,
  messagesOldestFirst,
  type MessagesInfiniteData,
  type MessagesResponse,
} from './useMessages';
import { MESSAGES_PAGE_SIZE } from '@/shared/constants';
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
