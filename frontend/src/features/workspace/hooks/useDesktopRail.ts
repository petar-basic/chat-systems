import { useEffect, useEffectEvent } from 'react';
import type { Workspace } from '@/stores/workspace';
import { instanceManager } from '@/lib/instances';
import type { DesktopCommand, DesktopShell, DesktopWorkspace } from '@/lib/desktopShell';

interface Rail {
  workspaces: Workspace[];
  deletedWorkspaces: Workspace[];
  unreadByWorkspace: Record<string, number>;
  currentWorkspaceId: string | undefined;
  onSelectWorkspace: (ws: Workspace) => void;
  onCreateWorkspace: (instanceUrl: string) => void;
  onLeaveWorkspace: (ws: Workspace) => void;
}

export function useDesktopRail(shell: DesktopShell | null, rail: Rail) {
  const origin = instanceManager.normalize(window.location.origin);
  const own = (ws: Workspace) => instanceManager.normalize(ws.instanceUrl) === origin;

  const toReport = (ws: Workspace, deleted: boolean): DesktopWorkspace => ({
    id: ws.id,
    name: ws.name,
    icon_url: ws.icon_url ?? null,
    unread: deleted ? 0 : (rail.unreadByWorkspace[ws.id] ?? 0),
    deleted,
  });
  const report = [
    ...rail.workspaces.filter(own).map((ws) => toReport(ws, false)),
    ...rail.deletedWorkspaces.filter(own).map((ws) => toReport(ws, true)),
  ];
  const reportKey = JSON.stringify([report, rail.currentWorkspaceId ?? null]);

  const sendReport = useEffectEvent(() => shell?.reportWorkspaces(report, rail.currentWorkspaceId ?? null));
  useEffect(() => {
    sendReport();
  }, [reportKey]);

  const answer = useEffectEvent((command: DesktopCommand) => {
    if (command.kind === 'create-workspace') {
      rail.onCreateWorkspace(origin);
      return;
    }
    if (command.kind !== 'open-workspace' && command.kind !== 'leave-workspace') return;
    const ws = [...rail.workspaces, ...rail.deletedWorkspaces].find((w) => w.id === command.workspaceId);
    if (!ws) return;
    rail.onSelectWorkspace(ws);
    if (command.kind === 'leave-workspace') rail.onLeaveWorkspace(ws);
  });
  useEffect(() => shell?.onCommand(answer), [shell]);
}
