import { describe, it, expect } from 'vitest';
import { canModerateChannel, canAddChannelMembers } from './channelPermissions';

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
