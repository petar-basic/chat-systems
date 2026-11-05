import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import type { DesktopShell } from '@/lib/desktopShell';
import i18n from '@/lib/i18n';
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

const server = vi.hoisted(() => ({ name: 'Northwind' }));

vi.mock('@/features/instance/hooks/useInstanceInfo', () => ({
  useInstanceInfo: () => ({ data: { name: server.name, sso_enabled: true, setup_required: false } }),
}));

function openInDesktop() {
  window.chatSystemsDesktop = {
    version: 1,
    reportWorkspaces: vi.fn(),
    notify: vi.fn(),
    onCommand: () => () => {},
  } satisfies DesktopShell;
}

function renderPage() {
  render(
    <MemoryRouter>
      <AddInstancePage />
    </MemoryRouter>,
  );
}

describe('AddInstancePage', () => {
  afterEach(async () => {
    delete window.chatSystemsDesktop;
    server.name = 'Northwind';
    await i18n.changeLanguage('en');
  });

  it('in the desktop app signs in to the server the window shows, without asking for it', () => {
    openInDesktop();
    renderPage();

    expect(screen.getByRole('heading', { name: 'Sign in to Northwind' })).toBeInTheDocument();
    expect(screen.queryByLabelText('Server URL')).not.toBeInTheDocument();
    expect(screen.queryByText('Advanced options')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign in' })).toBeInTheDocument();
    expect(screen.getByText('Sign in with SSO')).toBeInTheDocument();
  });

  it('in Japanese names the server as text, whatever it is called', async () => {
    server.name = '<img src=x onerror="alert(1)"> {{name}}';
    openInDesktop();
    await i18n.changeLanguage('ja');
    renderPage();

    expect(
      screen.getByRole('heading', { name: '<img src=x onerror="alert(1)"> {{name}} にサインイン' }),
    ).toBeInTheDocument();
    expect(document.querySelector('img')).toBeNull();
    expect(screen.getByRole('button', { name: 'サインイン' })).toBeInTheDocument();
  });

  it('in a browser lets you pick the server to connect to', () => {
    renderPage();

    expect(screen.getByRole('heading', { name: 'Welcome to Chat Systems' })).toBeInTheDocument();
    expect(screen.getByLabelText('Server URL')).toHaveValue(window.location.origin);
    expect(screen.getByRole('button', { name: 'Connect' })).toBeInTheDocument();
  });
});
