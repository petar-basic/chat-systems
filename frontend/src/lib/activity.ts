const IDLE_AFTER_MS = 2 * 60_000;
const INPUT_THROTTLE_MS = 1_000;
const INPUT_EVENTS = ['pointerdown', 'pointermove', 'keydown', 'wheel', 'touchstart'] as const;

type Listener = (active: boolean) => void;

const listeners = new Set<Listener>();
let active = false;
let started = false;
let lastInput = 0;
let idleTimer: ReturnType<typeof setTimeout> | undefined;

function set(next: boolean) {
  if (next === active) return;
  active = next;
  listeners.forEach((listener) => listener(next));
}

function sawInput() {
  if (document.visibilityState !== 'visible') return;
  const now = Date.now();
  if (active && now - lastInput < INPUT_THROTTLE_MS) return;
  lastInput = now;
  clearTimeout(idleTimer);
  idleTimer = setTimeout(() => set(false), IDLE_AFTER_MS);
  set(true);
}

function idle() {
  clearTimeout(idleTimer);
  set(false);
}

function visibilityChanged() {
  if (document.visibilityState === 'visible') {
    sawInput();
    return;
  }
  idle();
}

function start() {
  if (started || typeof window === 'undefined') return;
  started = true;
  for (const name of INPUT_EVENTS) window.addEventListener(name, sawInput, { passive: true, capture: true });
  document.addEventListener('visibilitychange', visibilityChanged);
  window.addEventListener('pagehide', idle);
  if (document.visibilityState === 'visible' && document.hasFocus()) sawInput();
}

export function isActive(): boolean {
  start();
  return active;
}

export function watchActivity(listener: Listener): () => void {
  start();
  listeners.add(listener);
  return () => listeners.delete(listener);
}
