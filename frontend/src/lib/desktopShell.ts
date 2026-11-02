export interface DesktopWorkspace {
  id: string;
  name: string;
  icon_url: string | null;
  unread: number;
  deleted: boolean;
}

export interface DesktopNotification {
  key: string;
  title: string;
  subtitle: string;
  body: string;
  workspace_id: string | null;
}

export type DesktopCommand =
  | { kind: 'open-workspace'; workspaceId: string }
  | { kind: 'create-workspace' }
  | { kind: 'open-notification'; key: string };

export interface DesktopShell {
  version: number;
  reportWorkspaces: (workspaces: DesktopWorkspace[], current: string | null) => void;
  notify: (notification: DesktopNotification) => void;
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
