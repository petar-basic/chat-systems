import { describe, expect, it } from 'vitest';
import type { InviteRequest } from '@/features/workspace/hooks/useInviteRequests';
import { guestOf, MAX_EMAILS, parseEmails, readyToSend, roleOf, termsFor } from './inviteRequestTerms';
import { NO_GUEST } from './guestTerms';

const request: InviteRequest = {
  id: 'r1',
  workspace_id: 'w1',
  requested_by: 'u1',
  requester_name: 'Rita',
  emails: ['a@x.test'],
  role: 'guest',
  guest_kind: 'single',
  guest_until: null,
  channel_ids: ['c1'],
  channel_names: ['vendor'],
  status: 'pending',
  created_at: '2026-11-13T10:00:00Z',
  expires_at: '2026-12-13T10:00:00Z',
};

describe('parseEmails', () => {
  it('splits on commas, spaces and new lines, once per address', () => {
    expect(parseEmails(' A@x.test, b@x.test\nc@x.test;a@x.test  ')).toEqual([
      'a@x.test',
      'b@x.test',
      'c@x.test',
    ]);
    expect(parseEmails('   ')).toEqual([]);
  });
});

describe('termsFor', () => {
  it('sends guest terms only for a guest', () => {
    expect(
      termsFor(['a@x.test'], 'member', { kind: 'single', channelIds: ['c1'], until: '2026-12-01' }),
    ).toEqual({
      emails: ['a@x.test'],
      role: 'member',
      channel_ids: [],
    });
    const guest = termsFor(['a@x.test'], 'guest', { kind: 'single', channelIds: ['c1'], until: '' });
    expect(guest).toEqual({
      emails: ['a@x.test'],
      role: 'guest',
      guest_kind: 'single',
      channel_ids: ['c1'],
      guest_until: null,
    });
  });
});

describe('a request read back into the form', () => {
  it('keeps its role and guest terms', () => {
    expect(roleOf(request)).toBe('guest');
    expect(guestOf(request)).toEqual({ kind: 'single', channelIds: ['c1'], until: '' });
    expect(roleOf({ ...request, role: 'member', guest_kind: null, channel_ids: [] })).toBe('member');
  });
});

describe('readyToSend', () => {
  it('needs one to twenty addresses, and a channel for a guest', () => {
    const many = Array.from({ length: MAX_EMAILS + 1 }, (_, i) => `p${i}@x.test`);
    expect(readyToSend(many, 'member', NO_GUEST)).toBe(false);
    expect(readyToSend(many.slice(1), 'member', NO_GUEST)).toBe(true);
    expect(readyToSend([], 'member', NO_GUEST)).toBe(false);
    expect(readyToSend(['a@x.test'], 'member', NO_GUEST)).toBe(true);
    expect(readyToSend(['a@x.test'], 'guest', NO_GUEST)).toBe(false);
    expect(readyToSend(['a@x.test'], 'guest', { ...NO_GUEST, channelIds: ['c1'] })).toBe(true);
  });
});
