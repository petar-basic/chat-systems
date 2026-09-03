import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from './api';
import { ApiError } from './errors';

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
      if (url.endsWith('/notifications/dnd')) {
        return fetchMock.mock.calls.filter((c) => urlOf(c).endsWith('/notifications/dnd')).length === 1
          ? secondGet.promise
          : secondRetry.promise;
      }
      throw new Error(`unexpected fetch: ${method} ${url}`);
    });

    const p1 = client.typed((c) => c.GET('/users/me'));
    const p2 = client.typed((c) => c.GET('/notifications/dnd'));
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
});
