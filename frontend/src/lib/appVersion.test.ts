import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { watchForNewBuild } from './appVersion';

function serving(version: string | null) {
  return vi.fn(async () =>
    version === null
      ? new Response('<!doctype html>', { status: 200 })
      : new Response(JSON.stringify({ version }), { status: 200 }),
  );
}

describe('watchForNewBuild', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('says so when the deployed build is not the one running, once', async () => {
    const onNew = vi.fn();
    const fetcher = serving('b2');
    watchForNewBuild('b1', onNew, fetcher);

    document.dispatchEvent(new Event('visibilitychange'));
    await vi.waitFor(() => expect(onNew).toHaveBeenCalledTimes(1));

    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(30 * 60_000);
    expect(onNew).toHaveBeenCalledTimes(1);
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it('checks on its own every few minutes', async () => {
    const onNew = vi.fn();
    const stop = watchForNewBuild('b1', onNew, serving('b2'));

    await vi.advanceTimersByTimeAsync(10 * 60_000);
    expect(onNew).toHaveBeenCalledTimes(1);
    stop();
  });

  it('stays quiet for the same build, or for something that is not a version file', async () => {
    const onNew = vi.fn();
    const stopSame = watchForNewBuild('b1', onNew, serving('b1'));
    const stopHtml = watchForNewBuild('b1', onNew, serving(null));

    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(10 * 60_000);
    expect(onNew).not.toHaveBeenCalled();
    stopSame();
    stopHtml();
  });

  it('does not watch a development build', async () => {
    const fetcher = serving('b2');
    watchForNewBuild('dev', vi.fn(), fetcher);

    document.dispatchEvent(new Event('visibilitychange'));
    await vi.advanceTimersByTimeAsync(10 * 60_000);
    expect(fetcher).not.toHaveBeenCalled();
  });
});
