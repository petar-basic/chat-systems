import { describe, it, expect } from 'vitest';
import { resolveCurrentWorkspaceRole } from './useCurrentWorkspaceRole';
import type { WorkspaceMember, WorkspaceRole } from '@/stores/workspace';

const member = (user_id: string, role: WorkspaceRole, workspace_id = 'ws1'): WorkspaceMember => ({
  workspace_id,
  user_id,
  role,
  joined_at: '2026-01-01T00:00:00Z',
  email: `${user_id}@test.local`,
  display_name: user_id,
  avatar_url: null,
});

describe('resolveCurrentWorkspaceRole', () => {
  it('reports "not resolved" while the user is still loading', () => {
    expect(resolveCurrentWorkspaceRole(undefined, 'ws1', [member('u1', 'owner')])).toEqual({
      role: null,
      isResolved: false,
    });
  });

  it('reports "not resolved" while the member list is still loading', () => {
    expect(resolveCurrentWorkspaceRole('u1', 'ws1', undefined)).toEqual({
      role: null,
      isResolved: false,
    });
  });

  it('reports "not resolved" when no workspace is selected', () => {
    expect(resolveCurrentWorkspaceRole('u1', null, [member('u1', 'owner')])).toEqual({
      role: null,
      isResolved: false,
    });
  });

  it('distinguishes a loaded list that does not contain the user from a loading list', () => {
    const notAMember = resolveCurrentWorkspaceRole('u1', 'ws1', [member('someone-else', 'owner')]);
    expect(notAMember).toEqual({ role: null, isResolved: true });

    const stillLoading = resolveCurrentWorkspaceRole('u1', 'ws1', undefined);
    expect(stillLoading.isResolved).toBe(false);
  });

  it('resolves the role once both sources have landed', () => {
    expect(resolveCurrentWorkspaceRole('u1', 'ws1', [member('u2', 'admin'), member('u1', 'member')])).toEqual(
      { role: 'member', isResolved: true },
    );
  });

  it('never carries a role across a workspace switch', () => {
    const owned = resolveCurrentWorkspaceRole('u1', 'ws1', [member('u1', 'owner')]);
    expect(owned.role).toBe('owner');

    // Switching workspaces: the new workspace's members have not arrived yet.
    // The old value must not survive — that is what let admin controls appear
    // in a workspace where the user is only a guest.
    const switching = resolveCurrentWorkspaceRole('u1', 'ws2', undefined);
    expect(switching).toEqual({ role: null, isResolved: false });

    const guest = resolveCurrentWorkspaceRole('u1', 'ws2', [member('u1', 'guest', 'ws2')]);
    expect(guest).toEqual({ role: 'guest', isResolved: true });
  });
});
