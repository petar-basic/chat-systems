import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';
import { globalEventBus } from './globalEventBus';
import { followInstanceRole } from './instanceRoleSync';
import { useInstanceStore, type InstanceUser } from '@/stores/instances';

const api = vi.hoisted(() => ({ refreshSession: vi.fn(), typed: vi.fn() }));

vi.mock('./instances', () => ({
  instanceManager: {
    normalize: (url: string) => url,
    get: () => ({ api }),
  },
}));

const INSTANCE = 'https://chat.example';
const ana: InstanceUser = {
  id: 'u-ana',
  email: 'ana@chat.example',
  display_name: 'Ana',
  avatar_url: null,
  instance_role: 'member',
};
const asServer = (instance_role: InstanceUser['instance_role']) => ({ ...ana, instance_role });

describe('a new instance role', () => {
  let stop: () => void;

  beforeEach(() => {
    api.refreshSession.mockReset();
    api.typed.mockReset();
    useInstanceStore.setState({ instances: [{ url: INSTANCE, user: ana }] });
    stop = followInstanceRole();
  });

  afterEach(() => stop());

  it('refreshes the session and the instance user, so Create workspace shows at once', async () => {
    api.refreshSession.mockResolvedValue({ access_token: 'new', user: asServer('workspace_creator') });

    globalEventBus.emit({ type: 'user.instance_role_changed', role: 'workspace_creator' }, INSTANCE);

    await waitFor(() =>
      expect(useInstanceStore.getState().instances[0].user.instance_role).toBe('workspace_creator'),
    );
    expect(api.refreshSession).toHaveBeenCalledTimes(1);
  });

  it('asks who it is when the refresh does not say', async () => {
    api.refreshSession.mockResolvedValue({ access_token: 'new' });
    api.typed.mockResolvedValue(asServer('admin'));

    globalEventBus.emit({ type: 'user.instance_role_changed', role: 'admin' }, INSTANCE);

    await waitFor(() => expect(useInstanceStore.getState().instances[0].user.instance_role).toBe('admin'));
  });

  it('leaves an instance it is not signed in to alone', async () => {
    globalEventBus.emit({ type: 'user.instance_role_changed', role: 'admin' }, 'https://elsewhere.example');
    await Promise.resolve();

    expect(api.refreshSession).not.toHaveBeenCalled();
    expect(useInstanceStore.getState().instances[0].user).toBe(ana);
  });
});
