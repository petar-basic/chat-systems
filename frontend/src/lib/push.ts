import { api } from './api';
import { logger } from './logger';

export interface PushState {
  supported: boolean;
  enabled: boolean;
  permission: NotificationPermission;
  subscribed: boolean;
}

/** A push service hands out binary keys; the API speaks base64url. */
function toBase64Url(buffer: ArrayBuffer | null): string {
  if (!buffer) return '';
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/**
 * Returns the buffer rather than the view: `applicationServerKey` wants a
 * `BufferSource`, and a `Uint8Array` over a possibly-shared buffer is not one as
 * far as the DOM types are concerned.
 */
function fromBase64Url(value: string): ArrayBuffer {
  const padded = value.replace(/-/g, '+').replace(/_/g, '/');
  const binary = atob(padded.padEnd(padded.length + ((4 - (padded.length % 4)) % 4), '='));
  const buffer = new ArrayBuffer(binary.length);
  const bytes = new Uint8Array(buffer);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return buffer;
}

export function pushSupported(): boolean {
  return 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window;
}

export async function registerWorker(): Promise<ServiceWorkerRegistration | null> {
  if (!pushSupported()) return null;
  try {
    return await navigator.serviceWorker.register('/sw.js');
  } catch (e) {
    logger.error('push', 'service worker registration failed', e);
    return null;
  }
}

export async function currentState(): Promise<PushState> {
  if (!pushSupported()) {
    return { supported: false, enabled: false, permission: 'denied', subscribed: false };
  }

  let enabled = false;
  try {
    const key = await api.get<{ public_key: string; enabled: boolean }>('/push/key');
    enabled = key.enabled;
  } catch {
    enabled = false;
  }

  const registration = await navigator.serviceWorker.getRegistration();
  const subscription = await registration?.pushManager.getSubscription();

  return {
    supported: true,
    enabled,
    permission: Notification.permission,
    subscribed: Boolean(subscription),
  };
}

/**
 * Asks the browser, then the push service, then registers the result here. The
 * permission prompt only appears in response to a click, which is why this is
 * never called on load.
 */
export async function subscribe(): Promise<boolean> {
  if (!pushSupported()) return false;

  const { public_key: publicKey, enabled } = await api.get<{
    public_key: string;
    enabled: boolean;
  }>('/push/key');
  if (!enabled || !publicKey) return false;

  const permission = await Notification.requestPermission();
  if (permission !== 'granted') return false;

  const registration = (await navigator.serviceWorker.getRegistration()) ?? (await registerWorker());
  if (!registration) return false;

  const subscription = await registration.pushManager.subscribe({
    userVisibleOnly: true,
    applicationServerKey: fromBase64Url(publicKey),
  });

  await api.post('/push/subscriptions', {
    endpoint: subscription.endpoint,
    keys: {
      p256dh: toBase64Url(subscription.getKey('p256dh')),
      auth: toBase64Url(subscription.getKey('auth')),
    },
    user_agent: navigator.userAgent.slice(0, 255),
  });

  return true;
}

/**
 * Unsubscribes here first. Dropping the browser subscription without telling the
 * server leaves a row that gets a request per notification until the push
 * service finally answers `410`.
 */
export async function unsubscribe(): Promise<void> {
  if (!pushSupported()) return;
  const registration = await navigator.serviceWorker.getRegistration();
  const subscription = await registration?.pushManager.getSubscription();
  if (!subscription) return;

  await api.delete('/push/subscriptions', { endpoint: subscription.endpoint }).catch(() => {});
  await subscription.unsubscribe();
}
