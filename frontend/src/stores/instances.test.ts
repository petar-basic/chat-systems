import { afterEach, describe, expect, it, vi } from 'vitest';
import { useInstanceStore, type InstanceConfig } from './instances';

const servers = vi.hoisted(() => ({ othersAllowed: false }));
vi.mock('../lib/instanceInfo', () => ({ otherServersAllowed: async () => servers.othersAllowed }));

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
