import { describe, it, expect, vi } from 'vitest';
import { reloadForNewChunks } from './chunkReload';

function page(start: number) {
  const stored = new Map<string, string>();
  let now = start;
  return {
    storage: {
      getItem: (key: string) => stored.get(key) ?? null,
      setItem: (key: string, value: string) => void stored.set(key, value),
    },
    reload: vi.fn(),
    now: () => now,
    wait: (ms: number) => {
      now += ms;
    },
  };
}

describe('reloadForNewChunks', () => {
  it('reloads once onto the new build, and not again straight away', () => {
    const tab = page(1_000_000);

    expect(reloadForNewChunks(tab)).toBe(true);
    expect(tab.reload).toHaveBeenCalledTimes(1);

    tab.wait(5_000);
    expect(reloadForNewChunks(tab), 'a chunk still missing after the reload is a real error').toBe(false);
    expect(tab.reload).toHaveBeenCalledTimes(1);
  });

  it('reloads again for the deploy after that', () => {
    const tab = page(1_000_000);
    reloadForNewChunks(tab);

    tab.wait(10 * 60_000);
    expect(reloadForNewChunks(tab)).toBe(true);
    expect(tab.reload).toHaveBeenCalledTimes(2);
  });

  it('does not loop when it cannot remember that it reloaded', () => {
    const tab = page(1_000_000);
    tab.storage.setItem = () => {
      throw new Error('storage is blocked');
    };

    expect(reloadForNewChunks(tab)).toBe(false);
    expect(tab.reload).not.toHaveBeenCalled();
  });
});
