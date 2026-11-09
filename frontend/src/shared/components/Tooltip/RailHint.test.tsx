import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import type { DesktopCommand, DesktopShell } from '@/lib/desktopShell';
import { RailHint } from './RailHint';

function shell() {
  const handlers = new Set<(command: DesktopCommand) => void>();
  const desktop: DesktopShell = {
    version: 1,
    reportWorkspaces: vi.fn(),
    notify: vi.fn(),
    onCommand: (handler) => {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
  };
  window.chatSystemsDesktop = desktop;
  return (command: DesktopCommand) => act(() => handlers.forEach((handler) => handler(command)));
}

describe('RailHint', () => {
  afterEach(() => {
    delete window.chatSystemsDesktop;
  });

  it('shows what the desktop rail is pointing at beside it, and hides it again', () => {
    const send = shell();
    render(<RailHint />);
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();

    send({ kind: 'rail-hint', hint: { text: 'Acme (⌘1)', y: 96, animate: true } });
    const tooltip = screen.getByRole('tooltip');
    expect(tooltip).toHaveTextContent('Acme (⌘1)');
    expect(tooltip).toHaveStyle({ top: '96px' });

    send({ kind: 'rail-hint', hint: null });
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('shows a workspace name as text', () => {
    const send = shell();
    render(<RailHint />);

    send({ kind: 'rail-hint', hint: { text: '<img src=x onerror="alert(1)">', y: 40, animate: false } });

    expect(screen.getByRole('tooltip')).toHaveTextContent('<img src=x onerror="alert(1)">');
    expect(screen.getByRole('tooltip').querySelector('img')).toBeNull();
  });

  it('draws nothing in a browser', () => {
    render(<RailHint />);
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });
});
