import { afterEach, describe, expect, it, vi } from 'vitest';
import i18n from '@/lib/i18n';
import { useInstanceStore, type InstanceConfig } from './instances';

const servers = vi.hoisted(() => ({ othersAllowed: false }));
vi.mock('../lib/instanceInfo', () => ({ otherServersAllowed: async () => servers.othersAllowed }));
const { toastError } = vi.hoisted(() => ({ toastError: vi.fn() }));
vi.mock('@/shared/components/Toast', () => ({ toast: { error: toastError } }));

const ELSEWHERE = 'https://elsewhere.example';

function savedElsewhere(): InstanceConfig {
  return {
    url: ELSEWHERE,
    user: {
      id: 'u-ana',
      email: 'ana@chat.example',
      display_name: 'Ana',
      avatar_url: null,
      instance_role: 'member',
    },
  };
}

describe('a server that keeps everyone to itself', () => {
  afterEach(() => {
    localStorage.clear();
    useInstanceStore.setState({ instances: [], activeInstanceUrl: null, hydrated: false, error: null });
  });

  it('refuses a sign-in to another server and says why', async () => {
    await expect(
      useInstanceStore.getState().addInstance(ELSEWHERE, 'ana@chat.example', 'secret'),
    ).rejects.toThrow('This server does not let you connect to other servers.');

    expect(useInstanceStore.getState().error).toBe('This server does not let you connect to other servers.');
    expect(useInstanceStore.getState().instances).toEqual([]);
    expect(localStorage.getItem('chat_instances')).toBeNull();
  });

  it('says why in Japanese', async () => {
    await i18n.changeLanguage('ja');
    try {
      await expect(
        useInstanceStore.getState().addInstance(ELSEWHERE, 'ana@chat.example', 'secret'),
      ).rejects.toThrow('このサーバーでは、ほかのサーバーへの接続は許可されていません。');
      expect(useInstanceStore.getState().error).toBe(
        'このサーバーでは、ほかのサーバーへの接続は許可されていません。',
      );
    } finally {
      await i18n.changeLanguage('en');
    }
  });

  it('drops a server added before, and forgets its session', async () => {
    localStorage.setItem('chat_instances', JSON.stringify([savedElsewhere()]));
    localStorage.setItem('chat_tokens', JSON.stringify({ [ELSEWHERE]: { access: 'a', refresh: 'r' } }));

    await useInstanceStore.getState().restoreInstances();

    expect(useInstanceStore.getState().instances).toEqual([]);
    expect(useInstanceStore.getState().hydrated).toBe(true);
    expect(JSON.parse(localStorage.getItem('chat_instances') ?? 'null')).toEqual([]);
    expect(JSON.parse(localStorage.getItem('chat_tokens') ?? 'null')).toEqual({});
  });
});

const ana = {
  id: 'u-ana',
  email: 'ana@chat.example',
  display_name: 'Ana',
  avatar_url: null,
  instance_role: 'member',
};

function fakeClients() {
  return {
    api: {
      typed: vi.fn().mockResolvedValue({ ...ana, access_token: 'a', refresh_token: 'r', user: ana }),
      setTokens: vi.fn(),
      refreshSession: vi.fn(),
      onSessionExpired: null as (() => void) | null,
      onTokensChanged: null,
    },
    ws: {
      onStatusChange: null,
      onSessionRevoked: null as ((reason: string) => void) | null,
      addReconnectListener: vi.fn(),
      connect: vi.fn(),
      disconnect: vi.fn(),
    },
  };
}

type Store = (typeof import('./instances'))['useInstanceStore'];

const ways: Record<string, (store: Store) => Promise<void>> = {
  'signing in with a password': (store) =>
    store.getState().addInstance(window.location.origin, 'ana@chat.example', 'secret'),
  'coming back to a saved server': (store) => {
    localStorage.setItem('chat_instances', JSON.stringify([{ url: window.location.origin, user: ana }]));
    return store.getState().restoreInstances();
  },
  'coming back from single sign-on': (store) => {
    window.history.replaceState({}, '', '/?sso=1');
    return store.getState().restoreInstances();
  },
};

describe.each([
  ['en', 'Your session expired. Please sign in again.', 'This session was ended. Please sign in again.'],
  [
    'ja',
    'サインインの有効期限が切れました。もう一度サインインしてください。',
    'この端末のサインインは終了しました。もう一度サインインしてください。',
  ],
])('when a session ends (%s)', (language, expired, revoked) => {
  afterEach(() => {
    localStorage.clear();
    window.history.replaceState({}, '', '/');
    vi.restoreAllMocks();
    toastError.mockReset();
  });

  it.each(Object.keys(ways))('says so after %s', async (way) => {
    vi.resetModules();
    const { default: freshI18n } = await import('@/lib/i18n');
    const { instanceManager } = await import('../lib/instances');
    const { useInstanceStore: store } = await import('./instances');
    await freshI18n.changeLanguage(language);
    const clients = fakeClients();
    vi.spyOn(instanceManager, 'get').mockReturnValue(
      clients as unknown as ReturnType<typeof instanceManager.get>,
    );

    await ways[way](store);
    expect(store.getState().instances.map((instance) => instance.url)).toEqual([window.location.origin]);

    clients.api.onSessionExpired?.();
    expect(toastError).toHaveBeenLastCalledWith(expired);
    clients.ws.onSessionRevoked?.('revoked');
    expect(toastError).toHaveBeenLastCalledWith(revoked);
    expect(toastError).toHaveBeenCalledTimes(2);
  });
});
