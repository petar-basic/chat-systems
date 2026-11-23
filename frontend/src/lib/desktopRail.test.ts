import { afterEach, describe, expect, it, vi } from 'vitest';
import { createElement, type ReactNode } from 'react';
import { act, renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { DesktopShell } from './desktopShell';
import { useEmptyRailWhenSignedOut, useReportOtherServers } from './desktopRail';
import { instanceInfoQuery } from './instanceInfo';
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

describe('the desktop app and other servers', () => {
  afterEach(() => {
    delete window.chatSystemsDesktop;
  });

  function info(otherServersAllowed: boolean) {
    return {
      name: 'Chat Systems',
      email_code_sign_in: true,
      max_upload_bytes: 1024,
      other_servers_allowed: otherServersAllowed,
      setup_required: false,
      sso_enabled: false,
      workspace_creation_restricted: false,
      api_version: 1,
      min_client_version: '5.0.0',
    };
  }

  it('hears whether this server lets people add others, and hears it again when that changes', async () => {
    const shell = { ...fakeShell(), reportOtherServers: vi.fn() };
    window.chatSystemsDesktop = shell;
    const client = new QueryClient();
    const key = instanceInfoQuery(window.location.origin).queryKey;
    const wrapper = ({ children }: { children: ReactNode }) =>
      createElement(QueryClientProvider, { client }, children);

    client.setQueryData(key, info(false));

    renderHook(() => useReportOtherServers(), { wrapper });
    expect(shell.reportOtherServers).toHaveBeenLastCalledWith(false);

    act(() => client.setQueryData(key, info(true)));
    await waitFor(() => expect(shell.reportOtherServers).toHaveBeenLastCalledWith(true));
    expect(shell.reportOtherServers).toHaveBeenCalledTimes(2);
  });
});
