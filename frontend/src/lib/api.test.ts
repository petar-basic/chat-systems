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

type FakeResponse = {
  status: number;
  ok: boolean;
  json: () => Promise<unknown>;
};

function makeResponse(status: number, body: unknown = {}): FakeResponse {
  return {
    status,
    ok: status >= 200 && status < 300,
    json: () => Promise.resolve(body),
  };
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
    const client = new ApiClient();

    const firstGet = deferred<FakeResponse>();
    const secondGet = deferred<FakeResponse>();
    const refreshCall = deferred<FakeResponse>();
    const firstRetry = deferred<FakeResponse>();
    const secondRetry = deferred<FakeResponse>();

    fetchMock.mockImplementation((url: string, init?: RequestInit) => {
      const method = init?.method ?? 'GET';
      if (url.endsWith('/auth/refresh') && method === 'POST') {
        return refreshCall.promise;
      }
      if (url.endsWith('/a')) {
        return fetchMock.mock.calls.filter((c) => String(c[0]).endsWith('/a')).length === 1
          ? firstGet.promise
          : firstRetry.promise;
      }
      if (url.endsWith('/b')) {
        return fetchMock.mock.calls.filter((c) => String(c[0]).endsWith('/b')).length === 1
          ? secondGet.promise
          : secondRetry.promise;
      }
      throw new Error(`unexpected fetch: ${method} ${url}`);
    });

    const p1 = client.get<{ id: string }>('/a');
    const p2 = client.get<{ id: string }>('/b');
    await flush();

    firstGet.resolve(makeResponse(401));
    secondGet.resolve(makeResponse(401));
    await flush();

    const refreshCalls = fetchMock.mock.calls.filter(
      (c) => String(c[0]).endsWith('/auth/refresh') && c[1]?.method === 'POST',
    );
    expect(refreshCalls.length).toBe(1);

    refreshCall.resolve(makeResponse(200, { ok: true }));
    await flush();

    firstRetry.resolve(makeResponse(200, { id: 'a' }));
    secondRetry.resolve(makeResponse(200, { id: 'b' }));

    await expect(p1).resolves.toEqual({ id: 'a' });
    await expect(p2).resolves.toEqual({ id: 'b' });

    const refreshCallsAfter = fetchMock.mock.calls.filter(
      (c) => String(c[0]).endsWith('/auth/refresh') && c[1]?.method === 'POST',
    );
    expect(refreshCallsAfter.length).toBe(1);
  });

  it('allows a fresh refresh on a later 401 after the in-flight one has settled', async () => {
    const client = new ApiClient();

    let getN = 0;
    let refreshN = 0;
    fetchMock.mockImplementation((url: string, init?: RequestInit) => {
      const method = init?.method ?? 'GET';
      if (url.endsWith('/auth/refresh') && method === 'POST') {
        refreshN += 1;
        return Promise.resolve(makeResponse(200, { ok: true }));
      }
      if (url.endsWith('/x')) {
        getN += 1;
        return Promise.resolve(getN % 2 === 1 ? makeResponse(401) : makeResponse(200, { id: getN }));
      }
      throw new Error(`unexpected fetch: ${method} ${url}`);
    });

    await expect(client.get('/x')).resolves.toEqual({ id: 2 });
    await expect(client.get('/x')).resolves.toEqual({ id: 4 });

    expect(refreshN).toBe(2);
  });

  it('surfaces a 401 ApiError and calls onSessionExpired when refresh fails', async () => {
    const client = new ApiClient();
    const onExpired = vi.fn();
    client.onSessionExpired = onExpired;

    fetchMock.mockImplementation((url: string, init?: RequestInit) => {
      const method = init?.method ?? 'GET';
      if (url.endsWith('/auth/refresh') && method === 'POST') {
        return Promise.resolve(makeResponse(401));
      }
      return Promise.resolve(makeResponse(401));
    });

    await expect(client.get('/secret')).rejects.toBeInstanceOf(ApiError);
    expect(onExpired).toHaveBeenCalledTimes(1);

    const refreshCalls = fetchMock.mock.calls.filter(
      (c) => String(c[0]).endsWith('/auth/refresh') && c[1]?.method === 'POST',
    );
    expect(refreshCalls.length).toBe(1);
  });
});
