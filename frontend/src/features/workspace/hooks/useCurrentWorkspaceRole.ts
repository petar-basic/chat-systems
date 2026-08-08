import { useMemo } from 'react';
import { useParams } from 'react-router';
import { useCurrentUser } from '@/hooks/queries/useAuth';
import { useWorkspaces, useWorkspaceMembers } from '@/hooks/queries/useWorkspaces';
import { useInstanceStore } from '@/stores/instances';
import { useWorkspaceStore, type WorkspaceMember, type WorkspaceRole } from '@/stores/workspace';

export interface CurrentWorkspaceRole {
  role: WorkspaceRole | null;
  /**
   * `false` while the user or the member list is still loading. Without this a
   * caller cannot tell "no role" from "not known yet", and every role-gated
   * control silently degrades to hidden on first paint.
   */
  isResolved: boolean;
}

export function resolveCurrentWorkspaceRole(
  userId: string | undefined,
  workspaceId: string | null,
  members: WorkspaceMember[] | undefined,
): CurrentWorkspaceRole {
  if (!userId || !workspaceId || !members) return { role: null, isResolved: false };
  const mine = members.find((m) => m.user_id === userId);
  return { role: (mine?.role as WorkspaceRole | undefined) ?? null, isResolved: true };
}

/**
 * The role is derived from live query data on every render rather than copied
 * into the store by an effect. A copy can be unset or belong to the workspace
 * you just navigated away from, and every role-gated control then vanishes or,
 * worse, appears where it should not.
 */
export function useCurrentWorkspaceRole(): CurrentWorkspaceRole {
  const { workspaceId: routeWorkspaceId } = useParams<{ workspaceId?: string }>();
  const storeWorkspaceId = useWorkspaceStore((s) => s.currentWorkspace?.id);
  const workspaceId = routeWorkspaceId ?? storeWorkspaceId ?? null;

  const { data: user } = useCurrentUser();
  const activeInstanceUrl = useInstanceStore((s) => s.activeInstanceUrl);
  const { data: workspaces = [] } = useWorkspaces();
  const instanceUrl =
    workspaces.find((w) => w.id === workspaceId)?.instanceUrl ?? activeInstanceUrl ?? undefined;

  const { data: members } = useWorkspaceMembers(workspaceId, instanceUrl);

  return useMemo(
    () => resolveCurrentWorkspaceRole(user?.id, workspaceId, members),
    [user?.id, workspaceId, members],
  );
}
