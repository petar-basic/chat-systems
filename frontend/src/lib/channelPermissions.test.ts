import { describe, it, expect } from 'vitest';
import {
  canModerateChannel,
  canAddChannelMembers,
  canPostInChannel,
  canBroadcastInChannel,
  creatableChannelKinds,
} from './channelPermissions';

describe('canModerateChannel', () => {
  it('lets workspace admins and owners moderate any channel', () => {
    expect(canModerateChannel('admin', undefined)).toBe(true);
    expect(canModerateChannel('owner', undefined)).toBe(true);
  });

  it('lets a channel admin moderate their own channel', () => {
    expect(canModerateChannel('member', 'admin')).toBe(true);
  });

  it('keeps plain members, guests and outsiders out', () => {
    expect(canModerateChannel('member', 'member')).toBe(false);
    expect(canModerateChannel('member', undefined)).toBe(false);
    expect(canModerateChannel('guest', 'admin')).toBe(false);
    expect(canModerateChannel('guest', 'member')).toBe(false);
    expect(canModerateChannel(null, undefined)).toBe(false);
  });
});

describe('canAddChannelMembers', () => {
  it('lets any workspace member add people to a public channel', () => {
    expect(canAddChannelMembers('member', undefined, 'public')).toBe(true);
    expect(canAddChannelMembers('member', 'member', 'public')).toBe(true);
  });

  it('requires belonging to a private channel', () => {
    expect(canAddChannelMembers('member', undefined, 'private')).toBe(false);
    expect(canAddChannelMembers('member', 'member', 'private')).toBe(true);
    expect(canAddChannelMembers('admin', undefined, 'private')).toBe(true);
  });

  it('never lets guests or signed-out users add people', () => {
    expect(canAddChannelMembers('guest', 'member', 'public')).toBe(false);
    expect(canAddChannelMembers('guest', undefined, 'public')).toBe(false);
    expect(canAddChannelMembers(null, undefined, 'public')).toBe(false);
  });
});

describe('canPostInChannel', () => {
  const locked = { post_policy: 'moderators' as const };

  it('lets everyone post in a channel nobody configured', () => {
    expect(canPostInChannel(undefined, 'member', undefined)).toBe(true);
    expect(canPostInChannel({ post_policy: 'everyone' }, 'guest', undefined)).toBe(true);
  });

  it('holds an announcement channel to the people who moderate it', () => {
    expect(canPostInChannel(locked, 'member', undefined)).toBe(false);
    expect(canPostInChannel(locked, 'guest', undefined)).toBe(false);
    expect(canPostInChannel(locked, 'member', 'admin')).toBe(true);
    expect(canPostInChannel(locked, 'admin', undefined)).toBe(true);
    expect(canPostInChannel(locked, 'owner', undefined)).toBe(true);
  });

  /// A guest made channel admin is still a guest — the server says so in
  /// `can_moderate`, and the two have to agree or the composer lies.
  it('does not promote a guest through channel admin', () => {
    expect(canPostInChannel(locked, 'guest', 'admin')).toBe(false);
  });
});

describe('canBroadcastInChannel', () => {
  it('lets anyone who may post broadcast by default', () => {
    expect(canBroadcastInChannel({}, 'members', 'member', 'member')).toBe(true);
    expect(canBroadcastInChannel(undefined, 'members', 'guest', undefined)).toBe(true);
  });

  it('keeps an admins-only rule to workspace and channel admins', () => {
    expect(canBroadcastInChannel({}, 'admins', 'member', 'member')).toBe(false);
    expect(canBroadcastInChannel({}, 'admins', 'guest', undefined)).toBe(false);
    expect(canBroadcastInChannel({}, 'admins', 'member', 'admin')).toBe(true);
    expect(canBroadcastInChannel({}, 'admins', 'admin', undefined)).toBe(true);
    expect(canBroadcastInChannel({}, 'admins', 'owner', 'member')).toBe(true);
  });

  it('refuses everyone in a channel that turned broadcasts off', () => {
    expect(canBroadcastInChannel({ allow_broadcast_mentions: false }, 'members', 'owner', 'admin')).toBe(
      false,
    );
  });
});

describe('creatableChannelKinds', () => {
  it('lets members create both kinds by default, and never a guest', () => {
    expect(creatableChannelKinds('member', {})).toEqual(['public', 'private']);
    expect(creatableChannelKinds('guest', {})).toEqual([]);
    expect(creatableChannelKinds(null, {})).toEqual([]);
  });

  it('follows the rule for each kind separately', () => {
    const settings = { public_channel_creators: 'admins' };
    expect(creatableChannelKinds('member', settings)).toEqual(['private']);
    expect(creatableChannelKinds('admin', settings)).toEqual(['public', 'private']);
    expect(
      creatableChannelKinds('member', {
        public_channel_creators: 'admins',
        private_channel_creators: 'admins',
      }),
    ).toEqual([]);
  });
});
