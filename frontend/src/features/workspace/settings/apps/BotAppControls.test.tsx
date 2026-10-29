import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BotAppControls } from './BotAppControls';
import type { BotApp, BotAppWithToken } from '@/features/workspace/hooks/useBotApps';

type Options = { onSuccess?: (app: BotAppWithToken) => void };

const rotateMutate = vi.fn();
const removeMutate = vi.fn();
const setScopesMutate = vi.fn();
const configureMutate = vi.fn();
const makeAppTokenMutate = vi.fn();

vi.mock('@/features/workspace/hooks/useBotApps', () => ({
  useBotAppActions: () => ({
    rotate: { mutate: rotateMutate, isPending: false, error: null },
    remove: { mutateAsync: removeMutate, isPending: false, error: null },
    setScopes: { mutateAsync: setScopesMutate, isPending: false, error: null },
    configure: { mutate: configureMutate, isPending: false, error: null },
    makeAppToken: { mutate: makeAppTokenMutate, isPending: false, error: null },
    revokeAppToken: { mutate: vi.fn(), isPending: false, error: null },
    revealSecret: { mutate: vi.fn(), isPending: false, error: null, data: undefined },
  }),
}));

const SCOPES = [
  { key: 'chat:write', text: 'Post, edit and delete its own messages where it is a member' },
  { key: 'commands', text: 'Own slash commands' },
  { key: 'users:read', text: 'Look people up in the workspace' },
];

const app = (over: Partial<BotApp> = {}): BotApp => ({
  id: 'app-1',
  workspace_id: 'ws-1',
  bot_user_id: 'bot-1',
  name: 'Deployer',
  description: 'Ships things',
  icon_url: null,
  scopes: ['chat:write', 'commands'],
  events: [],
  request_url: null,
  socket_mode: false,
  has_app_token: false,
  connected_sockets: 0,
  failing_since: null,
  last_failure: null,
  created_by: 'admin-1',
  created_at: '2026-10-29T00:00:00Z',
  updated_at: '2026-10-29T00:00:00Z',
  ...over,
});

const onRemoved = vi.fn();

function renderControls(over: Partial<BotApp> = {}) {
  render(<BotAppControls app={app(over)} scopes={SCOPES} onRemoved={onRemoved} />);
}

describe('BotAppControls', () => {
  beforeEach(() => {
    rotateMutate
      .mockReset()
      .mockImplementation((_id: string, options?: Options) =>
        options?.onSuccess?.({ ...app(), token: 'cbot-rotated' }),
      );
    removeMutate.mockReset().mockResolvedValue({ status: 'deleted' });
    setScopesMutate.mockReset().mockResolvedValue(app());
    configureMutate.mockReset();
    makeAppTokenMutate
      .mockReset()
      .mockImplementation((appId: string, options?: { onSuccess?: (made: unknown) => void }) =>
        options?.onSuccess?.({ app_id: appId, app_token: 'capp-once' }),
      );
    onRemoved.mockReset();
  });

  it('rotates the token and shows the new one once', () => {
    renderControls();
    fireEvent.click(screen.getByTestId('bot-app-rotate'));

    expect(rotateMutate.mock.calls[0][0]).toBe('app-1');
    expect(screen.getByTestId('bot-app-token-value')).toHaveTextContent('cbot-rotated');
    fireEvent.click(screen.getByTestId('bot-app-token-dismiss'));
    expect(screen.queryByText('cbot-rotated')).not.toBeInTheDocument();
  });

  it('asks again before an app gets a scope it did not have', async () => {
    renderControls();
    fireEvent.click(screen.getByTestId('bot-app-scope-edit-users:read'));
    fireEvent.click(screen.getByTestId('bot-app-scopes-save'));

    expect(setScopesMutate).not.toHaveBeenCalled();
    const consent = await screen.findByTestId('app-consent-permissions');
    expect(consent).toHaveTextContent('Look people up in the workspace');
    expect(consent).not.toHaveTextContent('Own slash commands');

    fireEvent.click(screen.getByTestId('app-consent-confirm'));
    await waitFor(() =>
      expect(setScopesMutate).toHaveBeenCalledWith({
        appId: 'app-1',
        scopes: ['chat:write', 'commands', 'users:read'],
      }),
    );
  });

  it('takes a scope away without asking', () => {
    renderControls();
    expect(screen.getByTestId('bot-app-scopes-save')).toBeDisabled();
    fireEvent.click(screen.getByTestId('bot-app-scope-edit-commands'));
    fireEvent.click(screen.getByTestId('bot-app-scopes-save'));

    expect(screen.queryByTestId('app-consent-permissions')).not.toBeInTheDocument();
    expect(setScopesMutate).toHaveBeenCalledWith({ appId: 'app-1', scopes: ['chat:write'] });
  });

  it('removes behind a confirmation', async () => {
    renderControls();
    fireEvent.click(screen.getByTestId('bot-app-remove'));
    expect(removeMutate).not.toHaveBeenCalled();

    fireEvent.click(await screen.findByTestId('bot-app-remove-dialog-confirm'));
    await waitFor(() => expect(onRemoved).toHaveBeenCalled());
    expect(removeMutate).toHaveBeenCalledWith('app-1');
  });

  it('subscribes an app to events delivered to its request URL', () => {
    renderControls();
    fireEvent.click(screen.getByTestId('bot-app-event-app_mention'));
    fireEvent.change(screen.getByLabelText(/Request URL/), {
      target: { value: ' https://bot.example.com/events ' },
    });
    fireEvent.click(screen.getByTestId('bot-app-events-save'));

    expect(configureMutate.mock.calls[0][0]).toEqual({
      appId: 'app-1',
      settings: {
        events: ['app_mention'],
        request_url: 'https://bot.example.com/events',
        socket_mode: false,
      },
    });
  });

  it('an app in socket mode gets an app-level token shown once and no request URL', () => {
    renderControls({ socket_mode: true, events: ['message'], connected_sockets: 2 });

    expect(screen.queryByLabelText(/Request URL/)).not.toBeInTheDocument();
    expect(screen.getByTestId('bot-app-socket-count')).toHaveTextContent('2 sockets open');
    fireEvent.click(screen.getByTestId('bot-app-app-token-make'));
    expect(makeAppTokenMutate.mock.calls[0][0]).toBe('app-1');
    expect(screen.getByTestId('bot-app-app-token-value')).toHaveTextContent('capp-once');
  });

  it('an app nothing reaches shows why', () => {
    renderControls({ failing_since: '2026-10-29T10:00:00Z', last_failure: 'The request URL answered 503' });

    expect(screen.getByTestId('bot-app-failing')).toHaveTextContent('answered 503');
  });
});
