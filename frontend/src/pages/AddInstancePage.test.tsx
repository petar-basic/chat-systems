import { describe, it, expect, vi, afterEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import type { DesktopShell } from '@/lib/desktopShell';
import i18n from '@/lib/i18n';
import { ApiError } from '@/lib/errors';
import AddInstancePage from './AddInstancePage';

const store = vi.hoisted(() => ({
  addInstance: vi.fn(),
  addInstanceWithPasskey: vi.fn(),
}));

vi.mock('@/stores/instances', () => ({
  useInstanceStore: () => ({
    addInstance: store.addInstance,
    addInstanceWithPasskey: store.addInstanceWithPasskey,
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
    vi.unstubAllGlobals();
    store.addInstance.mockReset();
    store.addInstanceWithPasskey.mockReset();
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

  it('signs in with a passkey and no email when the browser can make one', async () => {
    vi.stubGlobal('PublicKeyCredential', function PublicKeyCredential() {});
    store.addInstanceWithPasskey.mockResolvedValue(undefined);
    renderPage();

    fireEvent.click(screen.getByRole('button', { name: 'Sign in with a passkey' }));

    await waitFor(() =>
      expect(store.addInstanceWithPasskey).toHaveBeenCalledWith(window.location.origin, undefined),
    );
  });

  it('offers no passkey where the browser has none, or in the desktop app', () => {
    renderPage();
    expect(screen.queryByRole('button', { name: 'Sign in with a passkey' })).not.toBeInTheDocument();

    vi.stubGlobal('PublicKeyCredential', function PublicKeyCredential() {});
    openInDesktop();
    renderPage();
    expect(screen.queryByRole('button', { name: 'Sign in with a passkey' })).not.toBeInTheDocument();
  });

  it('after the password, lets a passkey stand in for the code', async () => {
    vi.stubGlobal('PublicKeyCredential', function PublicKeyCredential() {});
    store.addInstance.mockRejectedValue(new ApiError(409, 'totp_or_passkey_required'));
    store.addInstanceWithPasskey.mockResolvedValue(undefined);
    renderPage();

    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'ana@example.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'correct horse' } });
    fireEvent.click(screen.getByRole('button', { name: 'Connect' }));

    fireEvent.click(await screen.findByRole('button', { name: 'Use a passkey instead' }));

    await waitFor(() =>
      expect(store.addInstanceWithPasskey).toHaveBeenCalledWith(window.location.origin, {
        email: 'ana@example.com',
        password: 'correct horse',
      }),
    );
  });
});
