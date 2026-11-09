import { describe, it, expect } from 'vitest';
import { canCreateOn, mayCreateWorkspaces } from './useWorkspaceCreation';

describe('mayCreateWorkspaces', () => {
  it('lets anyone create while the instance does not restrict it', () => {
    expect(mayCreateWorkspaces({ workspace_creation_restricted: false }, 'member')).toBe(true);
    expect(mayCreateWorkspaces({}, 'member')).toBe(true);
  });

  it('lets only workspace creators and admins create once it does', () => {
    const restricted = { workspace_creation_restricted: true };
    expect(mayCreateWorkspaces(restricted, 'member')).toBe(false);
    expect(mayCreateWorkspaces(restricted, 'workspace_creator')).toBe(true);
    expect(mayCreateWorkspaces(restricted, 'admin')).toBe(true);
  });

  it('offers nothing before it knows the rule', () => {
    expect(mayCreateWorkspaces(undefined, 'admin')).toBe(false);
    expect(mayCreateWorkspaces({ workspace_creation_restricted: false }, undefined)).toBe(false);
  });
});

describe('canCreateOn', () => {
  it('finds the server however its address is written', () => {
    const creation = { 'https://chat.example.com': true, 'https://other.example.com': false };
    expect(canCreateOn(creation, 'https://chat.example.com/')).toBe(true);
    expect(canCreateOn(creation, 'https://other.example.com')).toBe(false);
    expect(canCreateOn(creation, 'https://unknown.example.com')).toBe(false);
    expect(canCreateOn(creation, undefined)).toBe(false);
  });
});
