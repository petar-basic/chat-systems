import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within, waitFor } from '@testing-library/react';
import { AppsSection } from './AppsSection';
import type { AppDirectory, InstalledApp } from '@/features/workspace/hooks/useAppDirectory';
import type { BotAppWithToken, NewBotApp } from '@/features/workspace/hooks/useBotApps';

const requestMutate = vi.fn();
const createMutate = vi.fn();
const enableMutate = vi.fn();
let directory: AppDirectory;

vi.mock('@/features/workspace/hooks/useAppDirectory', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/workspace/hooks/useAppDirectory')>()),
  useAppDirectory: () => ({ data: directory, isLoading: false, isError: false, refetch: vi.fn() }),
  useRequestApp: () => ({ mutate: requestMutate, isPending: false, error: null }),
}));
vi.mock('@/features/workspace/hooks/useApps', () => ({
  useApps: () => ({ data: { data: [] } }),
  useAppActions: () => ({}),
}));
vi.mock('@/features/workspace/hooks/useBotApps', () => ({
  useBotApps: () => ({ data: { data: [] } }),
  useBotAppActions: () => ({ create: { mutateAsync: createMutate, isPending: false, error: null } }),
}));
vi.mock('@/features/workspace/hooks/useHooks', () => ({
  useEnableHook: () => ({ mutate: enableMutate, isPending: false, error: null }),
  useRotateHook: () => ({ mutate: vi.fn(), isPending: false, error: null }),
  useDeleteHook: () => ({ mutateAsync: vi.fn(), isPending: false, error: null }),
}));

const installed = (over: Partial<InstalledApp>): InstalledApp => ({
  kind: 'incoming_webhook',
  id: 'h-ci',
  name: 'CI',
  description: null,
  added_by: 'u-ana',
  added_by_name: 'Ana',
  added_at: '2026-10-29T10:00:00Z',
  permissions: [{ key: 'post', text: 'Anyone with its URL can post messages into its channel' }],
  every_channel: false,
  channels: [{ id: 'ch-1', name: 'deploys' }],
  hidden_channels: 0,
  health: { state: 'ok' },
  app_id: null,
  ...over,
});

const rows = () => screen.getAllByTestId('app-directory-row');

beforeEach(() => {
  requestMutate.mockReset();
  enableMutate.mockReset();
  createMutate
    .mockReset()
    .mockImplementation(
      async (made: NewBotApp): Promise<BotAppWithToken> =>
        ({ name: made.name, token: 'cbot-once' }) as BotAppWithToken,
    );
  directory = {
    can_manage: true,
    available: [],
    bot_scopes: [
      { key: 'chat:write', text: 'Post, edit and delete its own messages where it is a member' },
      { key: 'users:read', text: 'Look people up in the workspace' },
    ],
    installed: [
      installed({
        hidden_channels: 1,
        health: { state: 'disabled', reason: 'Its URL kept failing', since: '2026-10-29T11:00:00Z' },
      }),
      installed({
        kind: 'workflow',
        id: 'w-1',
        name: 'Standup',
        permissions: [{ key: 'schedule', text: 'Runs on a schedule' }],
      }),
    ],
  };
});

describe('AppsSection', () => {
  it('lists everything installed with who added it, where, and whether it works', () => {
    render(<AppsSection workspaceId="ws-1" />);

    expect(rows()).toHaveLength(2);
    const ci = rows()[0];
    expect(ci).toHaveTextContent('CI');
    expect(ci).toHaveTextContent('Incoming webhook · Ana · #deploys, 1 private channel you are not in');
    expect(within(ci).getByTestId('app-health')).toHaveTextContent('Turned off');
    expect(within(rows()[1]).queryByTestId('app-health')).not.toBeInTheDocument();
  });

  it('opens an app page with what it can do, its channels and its health', () => {
    render(<AppsSection workspaceId="ws-1" />);
    fireEvent.click(rows()[0]);

    const page = screen.getByTestId('app-page');
    expect(page).toHaveTextContent('added by Ana');
    expect(screen.getByTestId('app-page-permissions')).toHaveTextContent('Anyone with its URL');
    expect(screen.getByTestId('app-page-channels')).toHaveTextContent(
      '#deploys, 1 private channel you are not in',
    );
    expect(screen.getByTestId('app-page-health')).toHaveTextContent('Its URL kept failing');

    fireEvent.click(screen.getByTestId('app-reinstall'));
    expect(enableMutate).toHaveBeenCalledWith('h-ci');

    fireEvent.click(screen.getByTestId('app-page-back'));
    expect(rows()).toHaveLength(2);
  });

  it('opens the page a link names', () => {
    render(<AppsSection workspaceId="ws-1" initialApp={{ kind: 'workflow', id: 'w-1' }} />);

    expect(screen.getByTestId('app-page')).toHaveTextContent('Standup');
    expect(screen.getByTestId('app-page-workflow')).toBeInTheDocument();
  });

  it('says so when the linked app is gone', () => {
    render(<AppsSection workspaceId="ws-1" initialApp={{ kind: 'bot_app', id: 'gone' }} />);

    expect(screen.getByTestId('app-directory-missing')).toBeInTheDocument();
    expect(rows()).toHaveLength(2);
  });

  it('shows a member the directory read-only and lets them ask for an app', () => {
    directory = { ...directory, can_manage: false };
    requestMutate.mockImplementation((_req: unknown, options?: { onSuccess?: () => void }) =>
      options?.onSuccess?.(),
    );
    render(<AppsSection workspaceId="ws-1" />);

    expect(screen.queryByTestId('app-directory-build-open')).not.toBeInTheDocument();
    fireEvent.change(screen.getByTestId('app-request-name'), { target: { value: ' PagerDuty ' } });
    fireEvent.change(screen.getByTestId('app-request-reason'), { target: { value: 'On-call alerts' } });
    fireEvent.click(screen.getByTestId('app-request-send'));
    expect(requestMutate.mock.calls[0][0]).toEqual({ name: 'PagerDuty', reason: 'On-call alerts' });
    expect(screen.getByTestId('app-request-sent')).toHaveTextContent('Asked the admins for PagerDuty');

    fireEvent.click(rows()[0]);
    expect(screen.getByTestId('app-page-read-only')).toBeInTheDocument();
    expect(screen.queryByTestId('app-reinstall')).not.toBeInTheDocument();
    expect(screen.queryByTestId('app-remove')).not.toBeInTheDocument();
  });

  it('makes no token for a new app until an admin agrees to its scopes', async () => {
    render(<AppsSection workspaceId="ws-1" />);
    expect(screen.queryByTestId('app-request')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId('app-directory-build-open'));
    fireEvent.change(screen.getByLabelText(/App name/), { target: { value: 'Echo' } });
    fireEvent.click(screen.getByTestId('bot-app-scope-users:read'));
    fireEvent.click(screen.getByTestId('bot-app-create'));

    expect(createMutate).not.toHaveBeenCalled();
    const consent = await screen.findByTestId('app-consent-permissions');
    expect(consent).toHaveTextContent('Post, edit and delete its own messages');
    expect(consent).toHaveTextContent('Look people up in the workspace');

    fireEvent.click(screen.getByTestId('app-consent-confirm'));
    await waitFor(() =>
      expect(createMutate).toHaveBeenCalledWith({
        name: 'Echo',
        description: '',
        scopes: ['chat:write', 'users:read'],
      }),
    );
    expect(await screen.findByTestId('bot-app-token-value')).toHaveTextContent('cbot-once');
  });

  it('a new app is not made when the admin does not agree', async () => {
    render(<AppsSection workspaceId="ws-1" />);
    fireEvent.click(screen.getByTestId('app-directory-build-open'));
    fireEvent.change(screen.getByLabelText(/App name/), { target: { value: 'Echo' } });
    fireEvent.click(screen.getByTestId('bot-app-create'));
    await screen.findByTestId('app-consent-permissions');

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByTestId('app-consent-permissions')).not.toBeInTheDocument());
    expect(createMutate).not.toHaveBeenCalled();
  });
});
