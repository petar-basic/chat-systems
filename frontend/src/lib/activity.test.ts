import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

let visibility: DocumentVisibilityState = 'visible';

async function freshTracker() {
  vi.resetModules();
  return import('./activity');
}

describe('activity', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    visibility = 'visible';
    vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => visibility);
    vi.spyOn(document, 'hasFocus').mockReturnValue(false);
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('is active while someone uses the window, and idle two minutes after they stop', async () => {
    const { watchActivity, isActive } = await freshTracker();
    const seen: boolean[] = [];
    watchActivity((active) => seen.push(active));
    expect(isActive()).toBe(false);

    window.dispatchEvent(new KeyboardEvent('keydown'));
    expect(isActive()).toBe(true);

    vi.advanceTimersByTime(119_000);
    window.dispatchEvent(new Event('pointermove'));
    vi.advanceTimersByTime(119_000);
    expect(isActive()).toBe(true);

    vi.advanceTimersByTime(2_000);
    expect(isActive()).toBe(false);
    expect(seen).toEqual([true, false]);
  });

  it('is idle the moment the tab is hidden, and ignores input it cannot see', async () => {
    const { isActive } = await freshTracker();
    expect(isActive()).toBe(false);
    window.dispatchEvent(new KeyboardEvent('keydown'));
    expect(isActive()).toBe(true);

    visibility = 'hidden';
    document.dispatchEvent(new Event('visibilitychange'));
    expect(isActive()).toBe(false);

    window.dispatchEvent(new KeyboardEvent('keydown'));
    expect(isActive()).toBe(false);

    visibility = 'visible';
    document.dispatchEvent(new Event('visibilitychange'));
    expect(isActive()).toBe(true);
  });

  it('is idle as soon as the page goes away', async () => {
    const { isActive } = await freshTracker();
    window.dispatchEvent(new KeyboardEvent('keydown'));
    expect(isActive()).toBe(true);

    window.dispatchEvent(new Event('pagehide'));
    expect(isActive()).toBe(false);
  });
});
