import { useNotificationPrefs } from '@/stores/notificationPrefs';
import { NOTIFICATION_SOUND_THROTTLE_MS, NOTIFICATION_AUTO_CLOSE_MS } from '@/shared/constants';
import { desktopShell, type DesktopShell } from '@/lib/desktopShell';

const MOST_PENDING_CLICKS = 50;

let permissionGranted = false;
let audioCtx: AudioContext | null = null;
let lastSoundAt = 0;
let nextKey = 0;
const pendingClicks = new Map<string, () => void>();
const listening = new WeakSet<DesktopShell>();

function getAudioContext(): AudioContext | null {
  try {
    if (!audioCtx) audioCtx = new AudioContext();
    if (audioCtx.state === 'suspended') void audioCtx.resume();
    return audioCtx;
  } catch {
    return null;
  }
}

if (typeof window !== 'undefined') {
  const unlock = () => {
    getAudioContext();
  };
  window.addEventListener('pointerdown', unlock, { once: true });
  window.addEventListener('keydown', unlock, { once: true });
}

export function playNotificationSound() {
  const ctx = getAudioContext();
  if (!ctx) return;

  try {
    const now = ctx.currentTime;

    const osc = ctx.createOscillator();
    const gain = ctx.createGain();

    osc.connect(gain);
    gain.connect(ctx.destination);

    osc.type = 'sine';
    osc.frequency.setValueAtTime(880, now);
    osc.frequency.exponentialRampToValueAtTime(660, now + 0.15);

    gain.gain.setValueAtTime(0, now);
    gain.gain.linearRampToValueAtTime(0.3, now + 0.01);
    gain.gain.exponentialRampToValueAtTime(0.001, now + 0.4);

    osc.start(now);
    osc.stop(now + 0.4);
  } catch {
    return;
  }
}

export function playMessageSound() {
  if (!useNotificationPrefs.getState().soundEnabled) return;
  const now = Date.now();
  if (now - lastSoundAt < NOTIFICATION_SOUND_THROTTLE_MS) return;
  lastSoundAt = now;
  playNotificationSound();
}

export function notificationPermission(): NotificationPermission | null {
  if (desktopShell()) return 'granted';
  return 'Notification' in window ? Notification.permission : null;
}

export async function requestNotificationPermission(): Promise<boolean> {
  if (desktopShell()) {
    permissionGranted = true;
    return true;
  }
  if (!('Notification' in window)) return false;

  if (Notification.permission === 'granted') {
    permissionGranted = true;
    return true;
  }

  if (Notification.permission === 'denied') return false;

  const result = await Notification.requestPermission();
  permissionGranted = result === 'granted';
  return permissionGranted;
}

function listenForClicks(shell: DesktopShell) {
  if (listening.has(shell)) return;
  listening.add(shell);
  shell.onCommand((command) => {
    if (command.kind !== 'open-notification') return;
    const onClick = pendingClicks.get(command.key);
    pendingClicks.delete(command.key);
    onClick?.();
  });
}

export interface NotificationOrigin {
  place: string;
  workspaceId: string | null;
}

const NOWHERE: NotificationOrigin = { place: '', workspaceId: null };

function notifyDesktop(
  shell: DesktopShell,
  title: string,
  body: string,
  origin: NotificationOrigin,
  onClick?: () => void,
) {
  const key = (nextKey++).toString(16);
  if (onClick) {
    listenForClicks(shell);
    pendingClicks.set(key, onClick);
    const oldest = pendingClicks.keys().next().value;
    if (pendingClicks.size > MOST_PENDING_CLICKS && oldest !== undefined) pendingClicks.delete(oldest);
  }
  shell.notify({ key, title, subtitle: origin.place, body, workspace_id: origin.workspaceId });
}

export function showNotification(
  title: string,
  body: string,
  tag: string,
  onClick?: () => void,
  origin: NotificationOrigin = NOWHERE,
) {
  if (document.hasFocus()) return;

  const shell = desktopShell();
  if (shell) {
    notifyDesktop(shell, title, body, origin, onClick);
    return;
  }

  if (!permissionGranted || !('Notification' in window)) return;
  if (Notification.permission !== 'granted') return;

  const notification = new Notification(title, {
    body,
    icon: '/favicon.ico',
    tag,
  });

  if (onClick) {
    notification.onclick = () => {
      window.focus();
      notification.close();
      onClick();
    };
  }

  setTimeout(() => notification.close(), NOTIFICATION_AUTO_CLOSE_MS);
}
