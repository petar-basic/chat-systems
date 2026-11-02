import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { DesktopCommand, DesktopShell } from '@/lib/desktopShell';
import { notificationPermission, requestNotificationPermission, showNotification } from './notifications';

function fakeShell() {
  const handlers = new Set<(command: DesktopCommand) => void>();
  const shell: DesktopShell = {
    version: 1,
    reportWorkspaces: vi.fn(),
    notify: vi.fn(),
    onCommand: (handler) => {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
  };
  return { shell, send: (command: DesktopCommand) => handlers.forEach((handler) => handler(command)) };
}

describe('notifications', () => {
  beforeEach(() => {
    vi.spyOn(document, 'hasFocus').mockReturnValue(false);
  });

  afterEach(() => {
    delete window.chatSystemsDesktop;
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('shows a browser notification when there is no desktop app', async () => {
    const shown = vi.fn();
    vi.stubGlobal(
      'Notification',
      Object.assign(
        function Notification(title: string, options: NotificationOptions) {
          shown(title, options.body);
        },
        { permission: 'granted' },
      ),
    );

    expect(await requestNotificationPermission()).toBe(true);
    showNotification('Maya', 'hello', 'general');

    expect(shown).toHaveBeenCalledWith('Maya', 'hello');
  });

  it('asks the desktop app, and opens the message once its notification is clicked', async () => {
    const { shell, send } = fakeShell();
    window.chatSystemsDesktop = shell;
    const onClick = vi.fn();

    expect(notificationPermission()).toBe('granted');
    expect(await requestNotificationPermission()).toBe(true);
    showNotification('Maya', 'hello', 'general', onClick, {
      place: 'Northwind · #general',
      workspaceId: 'ws-1',
    });

    expect(shell.notify).toHaveBeenCalledWith({
      key: expect.any(String),
      title: 'Maya',
      subtitle: 'Northwind · #general',
      body: 'hello',
      workspace_id: 'ws-1',
    });
    const { key } = vi.mocked(shell.notify).mock.calls[0][0];
    send({ kind: 'open-notification', key });
    send({ kind: 'open-notification', key });
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('stays quiet while the page has focus', () => {
    vi.mocked(document.hasFocus).mockReturnValue(true);
    const { shell } = fakeShell();
    window.chatSystemsDesktop = shell;

    showNotification('Maya', 'hello', 'general');

    expect(shell.notify).not.toHaveBeenCalled();
  });
});
