import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const get = vi.fn();
const post = vi.fn();
const del = vi.fn();

vi.mock('./api', () => ({
  api: {
    get: (...args: unknown[]) => get(...args),
    post: (...args: unknown[]) => post(...args),
    delete: (...args: unknown[]) => del(...args),
  },
}));

const requestPermission = vi.fn();
const subscribeOnManager = vi.fn();
const getSubscription = vi.fn();
const unsubscribeOnSubscription = vi.fn();

function keyOf(text: string) {
  return new TextEncoder().encode(text).buffer;
}

function installBrowser({ permission = 'default' }: { permission?: NotificationPermission } = {}) {
  const registration = {
    pushManager: {
      subscribe: subscribeOnManager,
      getSubscription,
    },
  };
  vi.stubGlobal('navigator', {
    serviceWorker: {
      register: vi.fn().mockResolvedValue(registration),
      getRegistration: vi.fn().mockResolvedValue(registration),
    },
    userAgent: 'Vitest',
  });
  vi.stubGlobal('PushManager', class {});
  vi.stubGlobal('Notification', { permission, requestPermission });
  vi.stubGlobal('window', globalThis);
}

describe('push subscription', () => {
  beforeEach(() => {
    get.mockReset();
    post.mockReset();
    del.mockReset();
    requestPermission.mockReset();
    subscribeOnManager.mockReset();
    getSubscription.mockReset();
    unsubscribeOnSubscription.mockReset();
    vi.resetModules();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('sends the keys the server needs, base64url encoded', async () => {
    installBrowser();
    get.mockResolvedValue({ public_key: 'BPublicKey', enabled: true });
    requestPermission.mockResolvedValue('granted');
    subscribeOnManager.mockResolvedValue({
      endpoint: 'https://push.example.test/abc',
      getKey: (name: string) => keyOf(name === 'p256dh' ? 'public-bytes' : 'auth-bytes'),
    });

    const { subscribe } = await import('./push');
    expect(await subscribe()).toBe(true);

    expect(post).toHaveBeenCalledWith(
      '/push/subscriptions',
      expect.objectContaining({
        endpoint: 'https://push.example.test/abc',
        keys: expect.objectContaining({
          p256dh: expect.stringMatching(/^[A-Za-z0-9_-]+$/),
          auth: expect.stringMatching(/^[A-Za-z0-9_-]+$/),
        }),
      }),
    );
  });

  it('registers nothing when the person refuses the browser prompt', async () => {
    installBrowser();
    get.mockResolvedValue({ public_key: 'BPublicKey', enabled: true });
    requestPermission.mockResolvedValue('denied');

    const { subscribe } = await import('./push');
    expect(await subscribe()).toBe(false);
    expect(subscribeOnManager).not.toHaveBeenCalled();
    expect(post).not.toHaveBeenCalled();
  });

  it('does not ask for permission on an instance with push switched off', async () => {
    installBrowser();
    get.mockResolvedValue({ public_key: '', enabled: false });

    const { subscribe } = await import('./push');
    expect(await subscribe()).toBe(false);
    expect(requestPermission).not.toHaveBeenCalled();
  });

  /// Dropping the browser subscription without telling the server leaves a row
  /// that costs a request per notification until the service finally answers 410.
  it('tells the server before it drops the browser subscription', async () => {
    installBrowser({ permission: 'granted' });
    const order: string[] = [];
    del.mockImplementation(async () => {
      order.push('server');
    });
    unsubscribeOnSubscription.mockImplementation(async () => {
      order.push('browser');
    });
    getSubscription.mockResolvedValue({
      endpoint: 'https://push.example.test/abc',
      unsubscribe: unsubscribeOnSubscription,
    });

    const { unsubscribe } = await import('./push');
    await unsubscribe();

    expect(del).toHaveBeenCalledWith('/push/subscriptions', {
      endpoint: 'https://push.example.test/abc',
    });
    expect(order).toEqual(['server', 'browser']);
  });
});
