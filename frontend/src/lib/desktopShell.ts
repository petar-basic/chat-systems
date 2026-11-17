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

export interface DesktopRailHint {
  text: string;
  y: number;
  animate: boolean;
}

export interface DesktopOverlayStroke {
  key: string;
  color: string;
  name: string;
  points: [number, number][];
  fade_at: number;
  fade_ms: number;
}

export type DesktopCommand =
  | { kind: 'open-workspace'; workspaceId: string }
  | { kind: 'leave-workspace'; workspaceId: string }
  | { kind: 'create-workspace' }
  | { kind: 'open-notification'; key: string }
  | { kind: 'rail-hint'; hint: DesktopRailHint | null };

export interface DesktopShell {
  version: number;
  reportWorkspaces: (workspaces: DesktopWorkspace[], current: string | null) => void;
  notify: (notification: DesktopNotification) => void;
  startSso?: (invite: string | null) => void;
  startPasskey?: () => void;
  reportTheme?: (theme: 'light' | 'dark') => void;
  reportColours?: (colours: DesktopColours | null) => void;
  reportCreation?: (allowed: boolean) => void;
  openDrawOverlay?: (width: number, height: number) => void;
  drawOnOverlay?: (stroke: DesktopOverlayStroke) => void;
  closeDrawOverlay?: () => void;
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
