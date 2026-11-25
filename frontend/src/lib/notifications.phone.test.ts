import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createChannel, sendNotification } from '@tauri-apps/plugin-notification';
import { showNotification } from './notifications';

const taps = vi.hoisted(() => ({ listeners: [] as ((event: unknown) => void)[] }));
const phone = vi.hoisted(() => ({ android: false }));

vi.mock('@/lib/phoneApp', () => ({
  inPhoneApp: true,
  get inAndroidApp() {
    return phone.android;
  },
}));
vi.mock('@tauri-apps/plugin-notification', () => ({
  Importance: { High: 4 },
  createChannel: vi.fn(async () => undefined),
  sendNotification: vi.fn(),
  onAction: vi.fn(async (listener: (event: unknown) => void) => {
    taps.listeners.push(listener);
    return { unregister: vi.fn() };
  }),
}));

function tap(id: number) {
  taps.listeners.forEach((listener) => listener({ actionId: 'tap', inputValue: null, notification: { id } }));
}

function allow(permission: NotificationPermission) {
  vi.stubGlobal('Notification', { permission });
}

describe('notifications in the phone app', () => {
  beforeEach(() => {
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden');
  });

  afterEach(() => {
    vi.mocked(sendNotification).mockClear();
    vi.mocked(createChannel).mockClear();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    phone.android = false;
  });

  it('shows a message through the app while it is in the background, and opens it once tapped', async () => {
    allow('granted');
    const onClick = vi.fn();

    showNotification('Maya', 'hello', 'general', onClick);

    await vi.waitFor(() =>
      expect(sendNotification).toHaveBeenCalledWith(
        expect.objectContaining({
          title: 'Maya',
          body: 'hello',
          id: expect.any(Number),
        }),
      ),
    );
    expect(createChannel).not.toHaveBeenCalled();
    expect(vi.mocked(sendNotification).mock.calls[0][0]).not.toHaveProperty('channelId');
    const { id } = vi.mocked(sendNotification).mock.calls[0][0] as { id: number };
    tap(id + 1);
    expect(onClick).not.toHaveBeenCalled();
    tap(id);
    expect(onClick).toHaveBeenCalledOnce();
    tap(id);
    expect(onClick).toHaveBeenCalledOnce();
  });

  it('puts messages in a channel of their own on Android, made once', async () => {
    phone.android = true;
    allow('granted');

    showNotification('Maya', 'hello', 'general');
    showNotification('Maya', 'again', 'general');

    await vi.waitFor(() => expect(sendNotification).toHaveBeenCalledTimes(2));
    expect(createChannel).toHaveBeenCalledOnce();
    expect(createChannel).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'messages', name: 'Messages', importance: 4 }),
    );
    expect(sendNotification).toHaveBeenLastCalledWith(
      expect.objectContaining({ body: 'again', channelId: 'messages', icon: 'ic_notification' }),
    );
  });

  it('stays quiet while the app is in front', () => {
    allow('granted');
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');

    showNotification('Maya', 'hello', 'general');

    expect(sendNotification).not.toHaveBeenCalled();
  });

  it('stays quiet until the phone allows notifications', () => {
    allow('default');

    showNotification('Maya', 'hello', 'general');

    expect(sendNotification).not.toHaveBeenCalled();
  });
});
