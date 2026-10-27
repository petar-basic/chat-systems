const CHECK_EVERY_MS = 10 * 60_000;

export const runningBuild: string | undefined = import.meta.env.VITE_BUILD_ID;

type Fetcher = (input: string, init?: RequestInit) => Promise<Response>;

export async function deployedBuild(fetcher: Fetcher = fetch): Promise<string | null> {
  try {
    const res = await fetcher('/version.json', { cache: 'no-store' });
    if (!res.ok) return null;
    const body = (await res.json()) as { version?: unknown };
    return typeof body.version === 'string' ? body.version : null;
  } catch {
    return null;
  }
}

export function watchForNewBuild(
  running: string | undefined,
  onNew: () => void,
  fetcher: Fetcher = fetch,
): () => void {
  if (!running || running === 'dev') return () => undefined;

  let stopped = false;
  const check = async () => {
    const deployed = await deployedBuild(fetcher);
    if (stopped || !deployed || deployed === running) return;
    stop();
    onNew();
  };
  const onReturn = () => {
    if (document.visibilityState === 'visible') void check();
  };
  const timer = setInterval(() => void check(), CHECK_EVERY_MS);
  document.addEventListener('visibilitychange', onReturn);
  window.addEventListener('online', onReturn);

  function stop() {
    stopped = true;
    clearInterval(timer);
    document.removeEventListener('visibilitychange', onReturn);
    window.removeEventListener('online', onReturn);
  }
  return stop;
}
