import { describe, expect, it } from 'vitest';
import type { Channel } from '@/stores/workspace';
import { dayOf, endingSoon, endOfDay, guestChannelChoices, toggleChannel, tomorrow } from './guestTerms';

const channel = (id: string, name: string, channel_type: string, is_archived = false) =>
  ({ id, name, channel_type, is_archived }) as unknown as Channel;

describe('guestTerms', () => {
  it('offers the open channels the admin is in and every public one, once each, by name', () => {
    const mine = [
      channel('a', 'vendor', 'public'),
      channel('b', 'legal', 'private'),
      channel('c', 'old', 'public', true),
      channel('d', '', 'dm'),
    ];
    const browsable = [
      { id: 'a', name: 'vendor', channel_type: 'public', is_archived: false },
      { id: 'e', name: 'ops', channel_type: 'public', is_archived: false },
      { id: 'f', name: 'gone', channel_type: 'private', is_archived: true },
    ] as Parameters<typeof guestChannelChoices>[1];
    expect(guestChannelChoices(mine, browsable).map((c) => c.name)).toEqual(['legal', 'ops', 'vendor']);
  });

  it('keeps one channel for a single-channel guest and any number for a multi-channel one', () => {
    const single = { kind: 'single' as const, channelIds: ['a'], until: '' };
    expect(toggleChannel(single, 'b', true).channelIds).toEqual(['b']);
    expect(toggleChannel(single, 'a', false).channelIds).toEqual([]);
    const multi = { kind: 'multi' as const, channelIds: ['a'], until: '' };
    expect(toggleChannel(multi, 'b', true).channelIds).toEqual(['a', 'b']);
    expect(toggleChannel(toggleChannel(multi, 'b', true), 'a', false).channelIds).toEqual(['b']);
  });

  it('ends access at the end of the chosen day and reads it back as that day', () => {
    const until = endOfDay('2026-12-01');
    expect(dayOf(until)).toBe('2026-12-01');
    expect(new Date(until).getHours()).toBe(23);
    expect(tomorrow(new Date(2026, 10, 30, 10))).toBe('2026-12-01');
  });

  it('counts as ending soon only within the next thirty days', () => {
    const now = new Date('2026-11-13T12:00:00Z');
    const inDays = (n: number) => new Date(now.getTime() + n * 86_400_000).toISOString();
    expect(endingSoon(inDays(3), now)).toBe(true);
    expect(endingSoon(inDays(30), now)).toBe(true);
    expect(endingSoon(inDays(31), now)).toBe(false);
    expect(endingSoon(inDays(-1), now)).toBe(false);
    expect(endingSoon(null, now)).toBe(false);
  });
});
