import { describe, it, expect, vi, afterEach } from 'vitest';
import type { DesktopShell } from '@/lib/desktopShell';

describe('theme store in the desktop app', () => {
  afterEach(() => {
    delete window.chatSystemsDesktop;
    localStorage.clear();
    vi.resetModules();
  });

  it('tells the app which theme the page shows, at start and on every change', async () => {
    const reportTheme = vi.fn();
    window.chatSystemsDesktop = {
      version: 1,
      reportWorkspaces: vi.fn(),
      notify: vi.fn(),
      onCommand: () => () => {},
      reportTheme,
    } satisfies DesktopShell;
    localStorage.setItem('chat_theme', 'light');

    const { useThemeStore } = await import('./theme');
    expect(reportTheme).toHaveBeenLastCalledWith('light');

    useThemeStore.getState().setMode('dark');
    expect(reportTheme).toHaveBeenLastCalledWith('dark');
  });
});
