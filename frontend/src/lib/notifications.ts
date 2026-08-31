import { useNotificationPrefs } from '@/stores/notificationPrefs';
import { NOTIFICATION_SOUND_THROTTLE_MS, NOTIFICATION_AUTO_CLOSE_MS } from '@/shared/constants';

let permissionGranted = false;
let audioCtx: AudioContext | null = null;
let lastSoundAt = 0;

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

export async function requestNotificationPermission(): Promise<boolean> {
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

export function showNotification(title: string, body: string, onClick?: () => void) {
  if (document.hasFocus()) return;

  if (!permissionGranted || !('Notification' in window)) return;
  if (Notification.permission !== 'granted') return;

  const notification = new Notification(title, {
    body,
    icon: '/favicon.ico',
    tag: 'chat-message',
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
