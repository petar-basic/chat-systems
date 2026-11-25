import { describe, expect, it } from 'vitest';
import type { Workspace } from '@/stores/workspace';
import { fromEveryServer } from './useWorkspaces';

const ACME = { id: 'w-acme', name: 'Acme', instanceUrl: 'https://one.example' } as Workspace;

describe('fromEveryServer', () => {
  it('lists what the servers that answered have when another cannot be reached', async () => {
    const list = await fromEveryServer(['https://one.example', 'https://two.example'], async (url) => {
      if (url === 'https://two.example') throw new TypeError('Load failed');
      return [ACME];
    });

    expect(list).toEqual([ACME]);
  });

  it('fails when no server answered, rather than saying there are no workspaces', async () => {
    const unreachable = new TypeError('Load failed');

    await expect(
      fromEveryServer(['https://one.example'], async () => {
        throw unreachable;
      }),
    ).rejects.toBe(unreachable);
  });

  it('lists nothing when every server answered with nothing', async () => {
    expect(await fromEveryServer(['https://one.example'], async () => [])).toEqual([]);
  });
});
