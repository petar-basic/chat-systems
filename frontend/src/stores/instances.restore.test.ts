import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { InstanceConfig } from './instances';

vi.mock('../lib/instanceInfo', () => ({ otherServersAllowed: async () => true }));
vi.mock('../lib/ws', () => ({
  WebSocketClient: class {
    onStatusChange = null;
    onSessionRevoked = null;
    connect = vi.fn();
    disconnect = vi.fn();
    addReconnectListener = vi.fn();
  },
}));

const ELSEWHERE = 'https://elsewhere.example';

const saved: InstanceConfig = {
  url: ELSEWHERE,
  user: {
    id: 'u-ana',
    email: 'ana@chat.example',
    display_name: 'Ana',
    avatar_url: null,
    instance_role: 'member',
  },
};

async function restore() {
  const { useInstanceStore } = await import('./instances');
  await useInstanceStore.getState().restoreInstances();
  return useInstanceStore.getState();
}

function storedTokens() {
  return JSON.parse(localStorage.getItem('chat_tokens') ?? 'null') as Record<string, { refresh: string }>;
}

describe('restoring a server on another origin', () => {
  beforeEach(() => {
    vi.resetModules();
    localStorage.setItem('chat_instances', JSON.stringify([saved]));
    localStorage.setItem('chat_tokens', JSON.stringify({ [ELSEWHERE]: { access: 'a', refresh: 'r' } }));
  });

  afterEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
  });

  it('keeps a server it cannot reach, with the user it had and its session', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Load failed')));

    const state = await restore();

    expect(state.hydrated).toBe(true);
    expect(state.instances).toEqual([saved]);
    expect(JSON.parse(localStorage.getItem('chat_instances') ?? 'null')).toEqual([saved]);
    expect(storedTokens()[ELSEWHERE]?.refresh).toBe('r');
  });

  it('keeps a server that fails the refresh', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('', { status: 502 })));

    const state = await restore();

    expect(state.instances).toEqual([saved]);
    expect(storedTokens()[ELSEWHERE]?.refresh).toBe('r');
  });

  it('forgets a server that refuses the session', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('', { status: 401 })));

    const state = await restore();

    expect(state.instances).toEqual([]);
    expect(storedTokens()).toEqual({});
  });
});
