type Spy = (...args: unknown[]) => unknown;

interface Spies {
  get?: Spy;
  post?: Spy;
  patch?: Spy;
  put?: Spy;
  delete?: Spy;
}

const ok = async (spy: Spy | undefined, ...args: unknown[]) => ({
  data: await spy?.(...args),
  response: { ok: true, status: 200 },
});

/// A stand-in for `ApiClient.typed` that routes each verb to a spy and hands
/// back what the spy resolves, the way the real client hands back the body.
export function typedApiMock(spies: Spies) {
  return {
    typed: (op: (client: unknown) => Promise<{ data: unknown }>) =>
      op({
        GET: (path: string) => ok(spies.get, path),
        POST: (path: string, init?: { body?: unknown }) => ok(spies.post, path, init?.body),
        PATCH: (path: string, init?: { body?: unknown }) => ok(spies.patch, path, init?.body),
        PUT: (path: string, init?: { body?: unknown }) => ok(spies.put, path, init?.body),
        DELETE: (path: string, init?: { body?: unknown }) => ok(spies.delete, path, init?.body),
      }).then((result) => result.data),
  };
}
