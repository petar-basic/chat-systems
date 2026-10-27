const RELOADED_AT = 'chunk-reload-at';
const AGAIN_AFTER_MS = 60_000;

interface Page {
  storage: Pick<Storage, 'getItem' | 'setItem'>;
  reload: () => void;
  now: () => number;
}

function browserPage(): Page {
  return { storage: sessionStorage, reload: () => window.location.reload(), now: Date.now };
}

export function reloadForNewChunks(page: Page = browserPage()): boolean {
  let last: number;
  try {
    last = Number(page.storage.getItem(RELOADED_AT)) || 0;
  } catch {
    return false;
  }
  if (page.now() - last < AGAIN_AFTER_MS) return false;
  try {
    page.storage.setItem(RELOADED_AT, String(page.now()));
  } catch {
    return false;
  }
  page.reload();
  return true;
}

export function reloadWhenChunksAreGone() {
  window.addEventListener('vite:preloadError', (event) => {
    if (reloadForNewChunks()) event.preventDefault();
  });
}
