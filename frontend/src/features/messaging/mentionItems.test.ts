import { describe, it, expect } from 'vitest';
import { buildMentionItems } from './mentionItems';
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

  it('keeps the ids the backend expands on', () => {
    const broadcasts = buildMentionItems([], [], false);

    expect(broadcasts.map((i) => i.id)).toEqual(['channel', 'here', 'everyone']);
  });
});
