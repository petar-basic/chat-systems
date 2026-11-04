export interface DesktopWorkspace {
  id: string;
  name: string;
  icon: string | null;
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

export interface DesktopColours {
  mode: 'light' | 'dark';
  rail: string;
  line: string;
  fg: string;
  muted: string;
  badge: string;
  badge_fg: string;
}

export type DesktopCommand =
  | { kind: 'open-workspace'; workspaceId: string }
  | { kind: 'leave-workspace'; workspaceId: string }
  | { kind: 'create-workspace' }
  | { kind: 'open-notification'; key: string };

export interface DesktopShell {
  version: number;
  reportWorkspaces: (workspaces: DesktopWorkspace[], current: string | null) => void;
  notify: (notification: DesktopNotification) => void;
  startSso?: (invite: string | null) => void;
  reportTheme?: (theme: 'light' | 'dark') => void;
  reportColours?: (colours: DesktopColours | null) => void;
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
