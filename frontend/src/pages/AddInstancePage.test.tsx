import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import type { DesktopShell } from '@/lib/desktopShell';
import AddInstancePage from './AddInstancePage';

vi.mock('@/stores/instances', () => ({
  useInstanceStore: () => ({
    addInstance: vi.fn(),
    instances: [],
    hydrated: true,
    loading: false,
    error: null,
    clearError: vi.fn(),
  }),
}));

vi.mock('@/features/instance/hooks/useInstanceInfo', () => ({
  useInstanceInfo: () => ({ data: { name: 'Northwind', sso_enabled: true, setup_required: false } }),
}));

function renderPage() {
  render(
    <MemoryRouter>
      <AddInstancePage />
    </MemoryRouter>,
  );
}

describe('AddInstancePage', () => {
  afterEach(() => {
    delete window.chatSystemsDesktop;
  });

  it('in the desktop app signs in to the server the window shows, without asking for it', () => {
    window.chatSystemsDesktop = {
      version: 1,
      reportWorkspaces: vi.fn(),
      notify: vi.fn(),
      onCommand: () => () => {},
    } satisfies DesktopShell;
    renderPage();

    expect(screen.getByRole('heading', { name: 'Sign in to Northwind' })).toBeInTheDocument();
    expect(screen.queryByLabelText('Server URL')).not.toBeInTheDocument();
    expect(screen.queryByText('Advanced options')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in' })).toBeInTheDocument();
    expect(screen.getByText('Sign in with SSO')).toBeInTheDocument();
  });

  it('in a browser lets you pick the server to connect to', () => {
    renderPage();

    expect(screen.getByRole('heading', { name: 'Welcome to Chat Systems' })).toBeInTheDocument();
    expect(screen.getByLabelText('Server URL')).toHaveValue(window.location.origin);
    expect(screen.getByRole('button', { name: 'Connect' })).toBeInTheDocument();
  });
});
