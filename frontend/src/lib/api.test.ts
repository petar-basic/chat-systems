import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from './api';
import { ApiError } from './errors';
import { useMediaTokenStore } from '@/stores/mediaTokens';
import { useServerSupportStore } from '@/stores/serverSupport';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function makeResponse(status: number, body: unknown = {}): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });
}

function requested(input: string | URL | Request, init?: RequestInit) {
  const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
  const method = init?.method ?? (input instanceof Request ? input.method : 'GET');
  return { url, method };
}

function urlOf(call: unknown[]): string {
  return requested(call[0] as string | URL | Request, call[1] as RequestInit | undefined).url;
}

function isRefreshCall(call: unknown[]): boolean {
  const { url, method } = requested(call[0] as string | URL | Request, call[1] as RequestInit | undefined);
  return url.endsWith('/auth/refresh') && method === 'POST';
}

const flush = () => new Promise<void>((r) => setTimeout(r, 0));

describe('ApiClient single-flight refresh', () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('issues exactly ONE POST /auth/refresh when two requests 401 concurrently, then retries both', async () => {
    const client = new ApiClient('http://api.test');

    const firstGet = deferred<Response>();
    const secondGet = deferred<Response>();
    const refreshCall = deferred<Response>();
    const firstRetry = deferred<Response>();
    const secondRetry = deferred<Response>();

    fetchMock.mockImplementation((input: string | URL | Request, init?: RequestInit) => {
      const { url, method } = requested(input, init);
      if (url.endsWith('/auth/refresh') && method === 'POST') {
        return refreshCall.promise;
      }
      if (url.endsWith('/users/me')) {
        return fetchMock.mock.calls.filter((c) => urlOf(c).endsWith('/users/me')).length === 1
          ? firstGet.promise
          : firstRetry.promise;
      }
      if (url.endsWith('/workspaces')) {
        return fetchMock.mock.calls.filter((c) => urlOf(c).endsWith('/workspaces')).length === 1
          ? secondGet.promise
          : secondRetry.promise;
      }
      throw new Error(`unexpected fetch: ${method} ${url}`);
    });

    const p1 = client.typed((c) => c.GET('/users/me'));
    const p2 = client.typed((c) => c.GET('/workspaces'));
    await flush();

    firstGet.resolve(makeResponse(401));
    secondGet.resolve(makeResponse(401));
    await flush();

    const refreshCalls = fetchMock.mock.calls.filter(isRefreshCall);
    expect(refreshCalls.length).toBe(1);

    refreshCall.resolve(makeResponse(200, { ok: true }));
    await flush();

    firstRetry.resolve(makeResponse(200, { id: 'a' }));
    secondRetry.resolve(makeResponse(200, { id: 'b' }));

    await expect(p1).resolves.toMatchObject({ id: 'a' });
    await expect(p2).resolves.toMatchObject({ id: 'b' });

    const refreshCallsAfter = fetchMock.mock.calls.filter(isRefreshCall);
    expect(refreshCallsAfter.length).toBe(1);
  });

  it('allows a fresh refresh on a later 401 after the in-flight one has settled', async () => {
    const client = new ApiClient('http://api.test');

    let getN = 0;
    let refreshN = 0;
    fetchMock.mockImplementation((input: string | URL | Request, init?: RequestInit) => {
      const { url, method } = requested(input, init);
      if (url.endsWith('/auth/refresh') && method === 'POST') {
        refreshN += 1;
        return Promise.resolve(makeResponse(200, { ok: true }));
      }
      if (url.endsWith('/users/me')) {
        getN += 1;
        return Promise.resolve(getN % 2 === 1 ? makeResponse(401) : makeResponse(200, { id: getN }));
      }
      throw new Error(`unexpected fetch: ${method} ${url}`);
    });

    await expect(client.typed((c) => c.GET('/users/me'))).resolves.toMatchObject({ id: 2 });
    await expect(client.typed((c) => c.GET('/users/me'))).resolves.toMatchObject({ id: 4 });

    expect(refreshN).toBe(2);
  });

  it('surfaces a 401 ApiError and calls onSessionExpired when refresh fails', async () => {
    const client = new ApiClient('http://api.test');
    const onExpired = vi.fn();
    client.onSessionExpired = onExpired;

    fetchMock.mockImplementation((input: string | URL | Request, init?: RequestInit) => {
      const { url, method } = requested(input, init);
      if (url.endsWith('/auth/refresh') && method === 'POST') {
        return Promise.resolve(makeResponse(401));
      }
      return Promise.resolve(makeResponse(401));
    });

    await expect(client.typed((c) => c.GET('/users/me'))).rejects.toBeInstanceOf(ApiError);
    expect(onExpired).toHaveBeenCalledTimes(1);

    const refreshCalls = fetchMock.mock.calls.filter(isRefreshCall);
    expect(refreshCalls.length).toBe(1);
  });

  it('treats a 401 from an anonymous call as the answer, without refreshing or expiring a session', async () => {
    const client = new ApiClient('http://api.test');
    const onExpired = vi.fn();
    client.onSessionExpired = onExpired;

    fetchMock.mockImplementation(() =>
      Promise.resolve(makeResponse(401, { error: 'Invalid email or password' })),
    );

    await expect(
      client.typed((c) => c.POST('/auth/login', { body: { email: 'a@b.test', password: 'wrong' } }), {
        anonymous: true,
      }),
    ).rejects.toMatchObject({ status: 401, message: 'Invalid email or password' });
    expect(onExpired).not.toHaveBeenCalled();
    expect(fetchMock.mock.calls.filter(isRefreshCall)).toHaveLength(0);
  });
});

describe('ApiClient and the server version', () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    useServerSupportStore.setState({ problems: {} });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('asks for every route under /api/v1', async () => {
    const client = new ApiClient('http://api.test');
    fetchMock.mockResolvedValue(makeResponse(200, { name: 'Chat' }));

    await client.typed((c) => c.GET('/instance/info'), { anonymous: true });

    expect(urlOf(fetchMock.mock.calls[0])).toBe('http://api.test/api/v1/instance/info');
  });

  it('says the server needs a newer app when it answers 426', async () => {
    const client = new ApiClient('http://api.test');
    fetchMock.mockResolvedValue(
      makeResponse(426, { error: 'This server needs a newer app', min_client_version: '6.0.0' }),
    );

    await expect(client.typed((c) => c.GET('/users/me'))).rejects.toMatchObject({
      status: 426,
      message: 'This server needs a newer app: Chat Systems 6.0.0 or later.',
    });
    expect(useServerSupportStore.getState().problems['http://api.test']).toEqual({
      kind: 'needsNewerApp',
      minVersion: '6.0.0',
    });
  });

  it('says the server is too old when it has no /api/v1 but answers at the old path', async () => {
    const client = new ApiClient('http://api.test');
    fetchMock.mockImplementation((input: string | URL | Request) => {
      const url = requested(input).url;
      if (url === 'http://api.test/api/instance/info')
        return Promise.resolve(makeResponse(200, { name: 'Old' }));
      return Promise.resolve(new Response('', { status: 404 }));
    });

    await expect(client.typed((c) => c.GET('/instance/info'), { anonymous: true })).rejects.toMatchObject({
      status: 404,
      message: 'This server runs an older Chat Systems. This app needs 5.0 or later there.',
    });
    expect(useServerSupportStore.getState().problems['http://api.test']).toEqual({ kind: 'serverTooOld' });
  });

  it('keeps a 404 with a reason as it is, without asking the old path', async () => {
    const client = new ApiClient('http://api.test');
    fetchMock.mockResolvedValue(makeResponse(404, { error: 'Channel not found' }));

    await expect(client.typed((c) => c.GET('/users/me'))).rejects.toMatchObject({
      status: 404,
      message: 'Channel not found',
    });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(useServerSupportStore.getState().problems).toEqual({});
  });

  it('says the server is too old when a request to another server never gets an answer', async () => {
    const client = new ApiClient('http://api.test');
    fetchMock.mockImplementation((input: string | URL | Request) => {
      const url = requested(input).url;
      if (url === 'http://api.test/api/instance/info')
        return Promise.resolve(makeResponse(200, { name: 'Old' }));
      return Promise.reject(new TypeError('Failed to fetch'));
    });

    await expect(client.typed((c) => c.GET('/users/me'))).rejects.toMatchObject({
      message: 'This server runs an older Chat Systems. This app needs 5.0 or later there.',
    });
  });
});

function credentialsOf(call: unknown[]): RequestCredentials | undefined {
  const [input, init] = call as [string | URL | Request, RequestInit | undefined];
  return init?.credentials ?? (input instanceof Request ? input.credentials : undefined);
}

function authorizationOf(call: unknown[]): string | null {
  const [input, init] = call as [string | URL | Request, RequestInit | undefined];
  if (input instanceof Request) return input.headers.get('Authorization');
  return new Headers(init?.headers).get('Authorization');
}

describe('ApiClient for a server on another origin', () => {
  let fetchMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    useMediaTokenStore.setState({ tokens: {} });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('sends no cookies, only its token', async () => {
    const client = new ApiClient('http://api.test');
    client.setTokens('access-1', 'refresh-1');
    fetchMock.mockResolvedValue(makeResponse(200, { id: 'a' }));

    await client.typed((c) => c.GET('/users/me'));

    expect(credentialsOf(fetchMock.mock.calls[0])).toBe('omit');
    expect(authorizationOf(fetchMock.mock.calls[0])).toBe('Bearer access-1');
  });

  it('signs out with the refresh token', async () => {
    const client = new ApiClient('http://api.test');
    client.setTokens('access-1', 'refresh-1');
    fetchMock.mockResolvedValue(makeResponse(200, { status: 'logged_out' }));

    await client.signOut();

    expect(urlOf(fetchMock.mock.calls[0])).toBe('http://api.test/api/v1/auth/logout');
    expect(authorizationOf(fetchMock.mock.calls[0])).toBe('Bearer refresh-1');
    expect(credentialsOf(fetchMock.mock.calls[0])).toBe('omit');
  });

  it('keeps a media token for its links and forgets it when it stops', async () => {
    vi.useFakeTimers();
    const client = new ApiClient('http://api.test');
    client.setTokens('access-1', 'refresh-1');
    fetchMock
      .mockResolvedValueOnce(makeResponse(200, { token: 'media-1', expires_in: 3600 }))
      .mockResolvedValueOnce(makeResponse(200, { token: 'media-2', expires_in: 3600 }));

    await client.keepMediaLinks();
    expect(useMediaTokenStore.getState().tokens).toEqual({ 'http://api.test': 'media-1' });

    await vi.advanceTimersByTimeAsync(3300 * 1000);
    expect(useMediaTokenStore.getState().tokens).toEqual({ 'http://api.test': 'media-2' });

    client.stopMediaLinks();
    expect(useMediaTokenStore.getState().tokens).toEqual({});
    await vi.advanceTimersByTimeAsync(3600 * 1000);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it('asks for no media token on its own origin', async () => {
    const client = new ApiClient();

    await client.keepMediaLinks();

    expect(fetchMock).not.toHaveBeenCalled();
    expect(useMediaTokenStore.getState().tokens).toEqual({});
  });
});
