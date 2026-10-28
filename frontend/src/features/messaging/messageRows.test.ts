import { describe, it, expect } from 'vitest';
import { boundaryReached, buildMessageRows } from './messageRows';

describe('the unread boundary', () => {
  const at = (i: number) => new Date(Date.UTC(2026, 0, 2, 12, i)).toISOString();
  const msg = (id: string, i: number, user = 'u1') => ({
    id,
    user_id: user,
    content: id,
    created_at: at(i),
  });

  it('draws the line once, after the last message that was read', () => {
    const rows = buildMessageRows([msg('a', 0), msg('b', 1), msg('c', 2)], 'a');
    const kinds = rows.map((r) => r.kind);
    expect(kinds.filter((k) => k === 'unread')).toHaveLength(1);
    expect(kinds.indexOf('unread')).toBe(kinds.indexOf('message') + 1);
  });

  it('draws nothing when everything has been read', () => {
    const rows = buildMessageRows([msg('a', 0), msg('b', 1)], 'b');
    expect(rows.some((r) => r.kind === 'unread')).toBe(false);
  });

  /// A boundary from an older page is not in the loaded messages; a line at the
  /// very top would claim everything is new.
  it('draws nothing when the boundary is not in the loaded page', () => {
    const rows = buildMessageRows([msg('b', 1), msg('c', 2)], 'older-message');
    expect(rows.some((r) => r.kind === 'unread')).toBe(false);
    expect(buildMessageRows([msg('b', 1)], null).some((r) => r.kind === 'unread')).toBe(false);
  });

  /// Otherwise the line disappears inside somebody's run of messages.
  it('breaks the grouping so the line is visible', () => {
    const rows = buildMessageRows([msg('a', 0), msg('b', 1)], 'a');
    const first = rows.find((r) => r.kind === 'message' && r.message.id === 'b');
    expect(first && first.kind === 'message' && first.grouped).toBe(false);
  });
});

describe('boundaryReached', () => {
  const at = (i: number) => new Date(Date.UTC(2026, 0, 2, 12, i)).toISOString();
  const msg = (id: string, i: number) => ({ id, created_at: at(i) });

  it('is reached once the boundary is loaded', () => {
    expect(boundaryReached([msg('a', 0), msg('b', 1)], { id: 'a', at: at(5) })).toBe(true);
  });

  it('keeps paging while everything loaded is newer than the read', () => {
    expect(boundaryReached([msg('c', 6), msg('d', 7)], { id: 'a', at: at(5) })).toBe(false);
  });

  it('stops once a page reaches back before the read, even when the boundary is not in the channel', () => {
    expect(boundaryReached([msg('b', 4), msg('c', 6)], { id: 'thread-reply', at: at(5) })).toBe(true);
  });

  it('pages on without a read time to compare against', () => {
    expect(boundaryReached([msg('b', 4)], { id: 'thread-reply', at: null })).toBe(false);
  });
});
