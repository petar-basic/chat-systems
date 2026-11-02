export interface DesktopWorkspace {
  id: string;
  name: string;
  icon_url: string | null;
  unread: number;
  deleted: boolean;
}

export type DesktopCommand = { kind: 'open-workspace'; workspaceId: string } | { kind: 'create-workspace' };

export interface DesktopShell {
  version: number;
  reportWorkspaces: (workspaces: DesktopWorkspace[], current: string | null) => void;
  onCommand: (handler: (command: DesktopCommand) => void) => () => void;
}

declare global {
  interface Window {
    chatSystemsDesktop?: DesktopShell;
  }
}

export function desktopShell(): DesktopShell | null {
  return window.chatSystemsDesktop ?? null;
}
