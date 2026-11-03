import { describe, it, expect, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { Workspace } from '@/stores/workspace';
import type { DesktopCommand, DesktopShell } from '@/lib/desktopShell';
import { useDesktopRail } from './useDesktopRail';

const { railIcon } = vi.hoisted(() => ({ railIcon: vi.fn() }));
vi.mock('@/lib/railIcon', () => ({ railIcon }));

function workspace(id: string, instanceUrl = window.location.origin): Workspace {
  return { id, name: `Workspace ${id}`, icon_url: null, instanceUrl } as Workspace;
}

function fakeShell() {
  let handler: ((command: DesktopCommand) => void) | null = null;
  const shell: DesktopShell = {
    version: 1,
    reportWorkspaces: vi.fn(),
    notify: vi.fn(),
    onCommand: (h) => {
      handler = h;
      return () => {
        handler = null;
      };
    },
  };
  return { shell, send: (command: DesktopCommand) => handler?.(command), subscribed: () => handler !== null };
}

describe('useDesktopRail', () => {
  it("reports only this server's workspaces, with their unread counts and the deleted ones", () => {
    const { shell } = fakeShell();
    renderHook(() =>
      useDesktopRail(shell, {
        workspaces: [workspace('a1'), workspace('b2', 'https://elsewhere.example.com')],
        deletedWorkspaces: [workspace('c3')],
        unreadByWorkspace: { a1: 4, b2: 9 },
        currentWorkspaceId: 'a1',
        onSelectWorkspace: vi.fn(),
        onCreateWorkspace: vi.fn(),
        onLeaveWorkspace: vi.fn(),
      }),
    );

    expect(shell.reportWorkspaces).toHaveBeenLastCalledWith(
      [
        { id: 'a1', name: 'Workspace a1', icon: null, unread: 4, deleted: false },
        { id: 'c3', name: 'Workspace c3', icon: null, unread: 0, deleted: true },
      ],
      'a1',
    );
  });

  it('hands the rail an icon the page drew, since the rail cannot load it signed out', async () => {
    railIcon.mockResolvedValue('data:image/png;base64,iVBORw0KGgo=');
    const { shell } = fakeShell();
    renderHook(() =>
      useDesktopRail(shell, {
        workspaces: [{ ...workspace('a1'), icon_url: '/api/files/download/icons/a1.png' }],
        deletedWorkspaces: [],
        unreadByWorkspace: {},
        currentWorkspaceId: 'a1',
        onSelectWorkspace: vi.fn(),
        onCreateWorkspace: vi.fn(),
        onLeaveWorkspace: vi.fn(),
      }),
    );

    expect(railIcon).toHaveBeenCalledWith('/api/files/download/icons/a1.png');
    await waitFor(() =>
      expect(shell.reportWorkspaces).toHaveBeenLastCalledWith(
        [
          {
            id: 'a1',
            name: 'Workspace a1',
            icon: 'data:image/png;base64,iVBORw0KGgo=',
            unread: 0,
            deleted: false,
          },
        ],
        'a1',
      ),
    );
  });

  it('opens the workspace the rail asks for and the create dialog on this server', () => {
    const { shell, send, subscribed } = fakeShell();
    const onSelectWorkspace = vi.fn();
    const onCreateWorkspace = vi.fn();
    const { unmount } = renderHook(() =>
      useDesktopRail(shell, {
        workspaces: [workspace('a1')],
        deletedWorkspaces: [],
        unreadByWorkspace: {},
        currentWorkspaceId: undefined,
        onSelectWorkspace,
        onCreateWorkspace,
        onLeaveWorkspace: vi.fn(),
      }),
    );

    act(() => send({ kind: 'open-workspace', workspaceId: 'a1' }));
    act(() => send({ kind: 'open-workspace', workspaceId: 'missing' }));
    act(() => send({ kind: 'create-workspace' }));

    expect(onSelectWorkspace).toHaveBeenCalledOnce();
    expect(onSelectWorkspace.mock.calls[0][0].id).toBe('a1');
    expect(onCreateWorkspace).toHaveBeenCalledWith(window.location.origin);
    unmount();
    expect(subscribed()).toBe(false);
  });

  it('opens a workspace before asking whether to leave it', () => {
    const { shell, send } = fakeShell();
    const onSelectWorkspace = vi.fn();
    const onLeaveWorkspace = vi.fn();
    renderHook(() =>
      useDesktopRail(shell, {
        workspaces: [workspace('a1')],
        deletedWorkspaces: [],
        unreadByWorkspace: {},
        currentWorkspaceId: undefined,
        onSelectWorkspace,
        onCreateWorkspace: vi.fn(),
        onLeaveWorkspace,
      }),
    );

    act(() => send({ kind: 'leave-workspace', workspaceId: 'a1' }));
    act(() => send({ kind: 'leave-workspace', workspaceId: 'missing' }));

    expect(onSelectWorkspace).toHaveBeenCalledOnce();
    expect(onLeaveWorkspace).toHaveBeenCalledOnce();
    expect(onLeaveWorkspace.mock.calls[0][0].id).toBe('a1');
  });

  it('does nothing outside the desktop app', () => {
    expect(() =>
      renderHook(() =>
        useDesktopRail(null, {
          workspaces: [workspace('a1')],
          deletedWorkspaces: [],
          unreadByWorkspace: {},
          currentWorkspaceId: 'a1',
          onSelectWorkspace: vi.fn(),
          onCreateWorkspace: vi.fn(),
          onLeaveWorkspace: vi.fn(),
        }),
      ),
    ).not.toThrow();
  });
});
