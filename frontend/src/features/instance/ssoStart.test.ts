import { describe, it, expect, vi, afterEach } from 'vitest';
import type { MouseEvent } from 'react';
import type { DesktopShell } from '@/lib/desktopShell';
import { startSsoInDesktop } from './ssoStart';

function click() {
  return { preventDefault: vi.fn() } as unknown as MouseEvent;
}

function shell(extra: Partial<DesktopShell> = {}): DesktopShell {
  return { version: 1, reportWorkspaces: vi.fn(), notify: vi.fn(), onCommand: () => () => {}, ...extra };
}

describe('startSsoInDesktop', () => {
  afterEach(() => {
    delete window.chatSystemsDesktop;
  });

  it('asks the desktop app to sign in through the system browser', () => {
    const startSso = vi.fn();
    window.chatSystemsDesktop = shell({ startSso });
    const event = click();

    startSsoInDesktop(event, 'invite-token');

    expect(event.preventDefault).toHaveBeenCalled();
    expect(startSso).toHaveBeenCalledWith('invite-token');
  });

  it('leaves the link alone in a browser or a desktop app too old to know SSO', () => {
    const event = click();
    startSsoInDesktop(event);

    window.chatSystemsDesktop = shell();
    startSsoInDesktop(event);

    expect(event.preventDefault).not.toHaveBeenCalled();
  });
});
