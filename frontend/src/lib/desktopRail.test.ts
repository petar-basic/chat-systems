import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import type { DesktopShell } from './desktopShell';
import { useEmptyRailWhenSignedOut } from './desktopRail';
import { useInstanceStore, type InstanceConfig } from '@/stores/instances';

function fakeShell(): DesktopShell {
  return { version: 1, reportWorkspaces: vi.fn(), notify: vi.fn(), onCommand: () => () => {} };
}

function signedIn(url: string): InstanceConfig {
  return {
    url,
    user: {
      id: 'u-ana',
      email: 'ana@chat.example',
      display_name: 'Ana',
      avatar_url: null,
      instance_role: 'member',
    },
  };
}

describe('the desktop rail after signing out', () => {
  afterEach(() => {
    delete window.chatSystemsDesktop;
    useInstanceStore.setState({ instances: [], hydrated: false });
  });

  it('tells the app this server has no workspaces once its session is gone', () => {
    const shell = fakeShell();
    window.chatSystemsDesktop = shell;
    useInstanceStore.setState({ instances: [signedIn(window.location.origin)], hydrated: true });
    renderHook(() => useEmptyRailWhenSignedOut());
    expect(shell.reportWorkspaces).not.toHaveBeenCalled();

    act(() => useInstanceStore.setState({ instances: [] }));

    expect(shell.reportWorkspaces).toHaveBeenCalledExactlyOnceWith([], null);
  });

  it('clears what the app kept from before a reload when the session did not come back', () => {
    const shell = fakeShell();
    window.chatSystemsDesktop = shell;
    useInstanceStore.setState({ instances: [], hydrated: false });
    renderHook(() => useEmptyRailWhenSignedOut());
    expect(shell.reportWorkspaces).not.toHaveBeenCalled();

    act(() =>
      useInstanceStore.setState({ instances: [signedIn('https://elsewhere.example')], hydrated: true }),
    );

    expect(shell.reportWorkspaces).toHaveBeenCalledExactlyOnceWith([], null);
  });
});
