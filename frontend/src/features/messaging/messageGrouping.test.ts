import { describe, it, expect } from 'vitest';
import { isGrouped, isNewDay, formatDaySeparator } from './messageGrouping';
import type { Message } from '@/stores/workspace';

const msg = (over: Partial<Message>): Message => ({
  id: 'm',
  channel_id: 'c',
  user_id: 'u1',
  content: 'hi',
  created_at: '2026-01-02T10:00:00Z',
  updated_at: '2026-01-02T10:00:00Z',
  deleted_at: null,
  thread_parent_id: null,
  reply_count: 0,
  is_pinned: false,
  ...over,
});

describe('isNewDay', () => {
  it('is true with no previous message', () => {
    expect(isNewDay(undefined, msg({}))).toBe(true);
  });
  it('is false within the same day, true across days', () => {
    const a = msg({ created_at: '2026-01-02T12:00:00Z' });
    const sameDay = msg({ created_at: '2026-01-02T13:00:00Z' });
    const nextDay = msg({ created_at: '2026-01-03T12:00:00Z' });
    expect(isNewDay(a, sameDay)).toBe(false);
    expect(isNewDay(a, nextDay)).toBe(true);
  });
});

describe('isGrouped', () => {
  const base = msg({ user_id: 'u1', created_at: '2026-01-02T10:00:00Z' });
  it('groups same author within the window', () => {
    expect(isGrouped(base, msg({ user_id: 'u1', created_at: '2026-01-02T10:03:00Z' }))).toBe(true);
  });
  it('does not group a different author', () => {
    expect(isGrouped(base, msg({ user_id: 'u2', created_at: '2026-01-02T10:01:00Z' }))).toBe(false);
  });
  it('does not group beyond the window', () => {
    expect(isGrouped(base, msg({ user_id: 'u1', created_at: '2026-01-02T10:30:00Z' }))).toBe(false);
  });
  it('does not group a failed message', () => {
    expect(isGrouped(base, msg({ user_id: 'u1', created_at: '2026-01-02T10:01:00Z', failed: true }))).toBe(
      false,
    );
  });
  it('does not group with no previous', () => {
    expect(isGrouped(undefined, base)).toBe(false);
  });
});

describe('formatDaySeparator', () => {
  const now = new Date('2026-01-02T12:00:00Z');
  it('labels today and yesterday', () => {
    expect(formatDaySeparator('2026-01-02T08:00:00Z', now)).toBe('Today');
    expect(formatDaySeparator('2026-01-01T08:00:00Z', now)).toBe('Yesterday');
  });
  it('labels older dates with a full date string', () => {
    const label = formatDaySeparator('2025-12-20T08:00:00Z', now);
    expect(label).not.toBe('Today');
    expect(label).not.toBe('Yesterday');
    expect(label.length).toBeGreaterThan(0);
  });
});
