import { describe, expect, it } from 'vitest';
import { allows, ruleOf } from './workspaceRules';

describe('ruleOf', () => {
  it('lets every member do what nobody restricted', () => {
    expect(ruleOf({}, 'public_channel_creators')).toBe('members');
    expect(ruleOf(null, 'workflow_builders')).toBe('members');
  });

  it('keeps invites with admins until someone opens them up', () => {
    expect(ruleOf({}, 'inviters')).toBe('admins');
    expect(ruleOf({ inviters: 'members' }, 'inviters')).toBe('members');
    expect(ruleOf({ inviters: 'nobody' }, 'inviters')).toBe('admins');
  });

  it('reads what was set', () => {
    expect(ruleOf({ broadcast_mentions: 'admins' }, 'broadcast_mentions')).toBe('admins');
  });
});

describe('allows', () => {
  it('never lets a guest', () => {
    expect(allows('members', 'guest')).toBe(false);
    expect(allows('members', 'member')).toBe(true);
    expect(allows('admins', 'member')).toBe(false);
    expect(allows('admins', 'owner')).toBe(true);
  });
});
