/**
 * Push, and the bare minimum to be installable. There is deliberately no
 * caching: an app shell served from a cache is an app that keeps running a
 * version the server has already replaced, and nobody asked for offline. The
 * fetch handler below exists only because Chrome will not offer to install a
 * site whose worker has none; it hands every request straight to the network.
 */

self.addEventListener('install', () => {
  // A push subscription is useless until the worker controlling the page can
  // receive it, and the default is to wait for every tab to close first.
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(self.clients.claim());
});

self.addEventListener('fetch', () => {});

self.addEventListener('push', (event) => {
  if (!event.data) return;

  let payload;
  try {
    payload = event.data.json();
  } catch {
    return;
  }

  const title = payload.title || 'Chat Systems';
  const options = {
    body: payload.body || '',
    icon: '/icons/icon-192.png',
    badge: '/icons/icon-192.png',
    // Replaces rather than stacks: five notifications from one channel is five
    // reasons to turn them off.
    tag: payload.channel_id || 'chat-systems',
    renotify: true,
    data: {
      workspace_id: payload.workspace_id,
      channel_id: payload.channel_id,
      message_id: payload.message_id,
    },
  };

  event.waitUntil(
    (async () => {
      await self.registration.showNotification(title, options);
      if (typeof payload.badge_count === 'number' && 'setAppBadge' in navigator) {
        try {
          await navigator.setAppBadge(payload.badge_count);
        } catch {
          // Badging is best effort; a browser without it still shows the notification.
        }
      }
    })(),
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const data = event.notification.data || {};

  let target = '/app';
  if (data.workspace_id && data.channel_id && data.message_id) {
    target = `/app/${data.workspace_id}/${data.channel_id}/${data.message_id}`;
  } else if (data.workspace_id) {
    target = `/app/${data.workspace_id}`;
  }

  event.waitUntil(
    (async () => {
      const windows = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
      // Focusing the tab they already have open beats opening a second copy of
      // the app next to it.
      for (const client of windows) {
        if (new URL(client.url).origin === self.location.origin) {
          await client.focus();
          if ('navigate' in client) await client.navigate(target);
          return;
        }
      }
      await self.clients.openWindow(target);
    })(),
  );
});
