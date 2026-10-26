import { describe, it, expect } from 'vitest';
import { asksBeforeBroadcast, buildMentionItems } from './mentionItems';
import type { Channel, WorkspaceMember } from '@/stores/workspace';

const members = [
  { user_id: 'u-1', display_name: 'Alice Johnson', email: 'alice@dev.local' },
  { user_id: 'u-2', display_name: null, email: 'bob@dev.local' },
] as WorkspaceMember[];

const channels = [{ id: 'ch-1', name: 'general' }] as Channel[];

describe('buildMentionItems', () => {
  it('offers @channel, @here and @everyone above the people', () => {
    const items = buildMentionItems(members, channels, false);

    expect(items.slice(0, 3).map((i) => i.label)).toEqual(['channel', 'here', 'everyone']);
    expect(items.slice(0, 3).every((i) => i.type === 'broadcast')).toBe(true);
  });

  it('leaves broadcasts out of direct messages, where they mean nothing', () => {
    const items = buildMentionItems(members, channels, true);

    expect(items.some((i) => i.type === 'broadcast')).toBe(false);
  });

  it('falls back to the email when a member has no display name', () => {
    const items = buildMentionItems(members, channels, true);

    expect(items.map((i) => i.label)).toEqual(['Alice Johnson', 'bob@dev.local', 'general']);
  });

  it('leaves broadcasts out for someone the channel or workspace would refuse', () => {
    const items = buildMentionItems(members, channels, false, [], false);

    expect(items.some((i) => i.type === 'broadcast')).toBe(false);
    expect(items.map((i) => i.label)).toEqual(['Alice Johnson', 'bob@dev.local', 'general']);
  });

  it('keeps the ids the backend expands on', () => {
    const broadcasts = buildMentionItems([], [], false);

    expect(broadcasts.map((i) => i.id)).toEqual(['channel', 'here', 'everyone']);
  });
});

describe('group mentions', () => {
  const groups = [
    {
      id: 'g1',
      handle: 'backend',
      name: 'Backend',
      description: null,
      member_count: 3,
      is_member: true,
    },
  ];

  it('offers a group with the prefix that keeps it out of the user id space', () => {
    const items = buildMentionItems([], [], false, groups);
    const group = items.find((i) => i.type === 'group');
    expect(group).toMatchObject({ id: 'group:g1', label: 'backend', hint: '3 people' });
  });

  it('offers no group in a DM, where there is no channel to fan out through', () => {
    const items = buildMentionItems([], [], true, groups);
    expect(items.some((i) => i.type === 'group')).toBe(false);
  });
});

describe('asksBeforeBroadcast', () => {
  it('asks before notifying a channel of more than 25 people', () => {
    expect(asksBeforeBroadcast('@[channel](channel) lunch', 26)).toBe(true);
    expect(asksBeforeBroadcast('@[here](here) anyone?', 140)).toBe(true);
    expect(asksBeforeBroadcast('@everyone hello', 140)).toBe(true);
  });

  it('sends straight away in a small channel or without a broadcast', () => {
    expect(asksBeforeBroadcast('@[channel](channel) lunch', 25)).toBe(false);
    expect(asksBeforeBroadcast('@[Ana](u-ana) lunch', 140)).toBe(false);
    expect(asksBeforeBroadcast('mail me at team@channel.dev', 140)).toBe(false);
    expect(asksBeforeBroadcast('@channels are fun', 140)).toBe(false);
  });
});
