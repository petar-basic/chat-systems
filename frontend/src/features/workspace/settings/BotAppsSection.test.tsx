import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BotAppsSection } from './BotAppsSection';
import type { BotApp, BotAppWithToken, NewBotApp } from '@/features/workspace/hooks/useBotApps';

type Options = { onSuccess?: (app: BotAppWithToken) => void };

const createMutate = vi.fn();
const rotateMutate = vi.fn();
const removeMutate = vi.fn();
let apps: BotApp[] = [];

vi.mock('@/features/workspace/hooks/useBotApps', () => ({
  useBotApps: () => ({ data: { data: apps }, isLoading: false, isError: false }),
  useBotAppActions: () => ({
    create: { mutate: createMutate, isPending: false, error: null },
    rotate: { mutate: rotateMutate, isPending: false, error: null },
    remove: { mutateAsync: removeMutate, isPending: false, error: null },
  }),
}));

const app = (over: Partial<BotApp> = {}): BotApp => ({
  id: 'app-1',
  workspace_id: 'ws-1',
  bot_user_id: 'bot-1',
  name: 'Deployer',
  description: 'Ships things',
  icon_url: null,
  scopes: ['chat:write', 'commands'],
  created_by: 'admin-1',
  created_at: '2026-10-29T00:00:00Z',
  updated_at: '2026-10-29T00:00:00Z',
  ...over,
});

describe('BotAppsSection', () => {
  beforeEach(() => {
    apps = [];
    createMutate
      .mockReset()
      .mockImplementation((made: NewBotApp, options?: Options) =>
        options?.onSuccess?.({ ...app({ name: made.name, scopes: made.scopes }), token: 'cbot-once' }),
      );
    rotateMutate
      .mockReset()
      .mockImplementation((_id: string, options?: Options) =>
        options?.onSuccess?.({ ...app(), token: 'cbot-rotated' }),
      );
    removeMutate.mockReset().mockResolvedValue({ status: 'deleted' });
  });

  it('makes an app with the chosen scopes and shows its token once', () => {
    render(<BotAppsSection workspaceId="ws-1" isAdmin />);
    fireEvent.change(screen.getByLabelText(/App name/), { target: { value: 'Echo' } });
    fireEvent.click(screen.getByTestId('bot-app-scope-users:read'));
    fireEvent.click(screen.getByTestId('bot-app-create'));

    expect(createMutate.mock.calls[0][0]).toEqual({
      name: 'Echo',
      description: '',
      scopes: ['chat:write', 'users:read'],
    });
    expect(screen.getByTestId('bot-app-token-value')).toHaveTextContent('cbot-once');

    fireEvent.click(screen.getByTestId('bot-app-token-dismiss'));
    expect(screen.queryByText('cbot-once')).not.toBeInTheDocument();
  });

  it('a new token replaces the old one the same way', () => {
    apps = [app()];
    render(<BotAppsSection workspaceId="ws-1" isAdmin />);
    fireEvent.click(screen.getByTestId('bot-app-rotate'));

    expect(rotateMutate.mock.calls[0][0]).toBe('app-1');
    expect(screen.getByTestId('bot-app-token-value')).toHaveTextContent('cbot-rotated');
  });

  it('removes behind a confirmation', async () => {
    apps = [app()];
    render(<BotAppsSection workspaceId="ws-1" isAdmin />);
    fireEvent.click(screen.getByTestId('bot-app-remove'));
    expect(removeMutate).not.toHaveBeenCalled();

    fireEvent.click(await screen.findByTestId('bot-app-remove-dialog-confirm'));
    expect(removeMutate).toHaveBeenCalledWith('app-1');
  });

  it('a member sees the apps and their scopes, and nothing to change them with', () => {
    apps = [app()];
    render(<BotAppsSection workspaceId="ws-1" isAdmin={false} />);

    expect(screen.getByTestId('bot-app-scopes')).toHaveTextContent('commands');
    expect(screen.queryByTestId('bot-app-form')).not.toBeInTheDocument();
    expect(screen.queryByTestId('bot-app-rotate')).not.toBeInTheDocument();
  });
});
