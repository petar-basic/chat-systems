# CS-108 — Notifications, badge and tray

**Wave:** 29 — a desktop app (2026-11-02)
**Area:** desktop · frontend
**Blocked by:** CS-107
**Blocks:** —

## Problem

Inside a web view, `Notification` and `navigator.setAppBadge` are missing or do nothing,
so the app would be quieter than the browser tab it replaces.

## Approach

- The app exposes a few commands to the page: `notify`, `set_badge`, `set_tray_unread`,
  `open_external`. Tauri capabilities allow them only from the origins the person added,
  and nothing else is exposed: no file system, no shell.
- `frontend/src/lib/notifications.ts` and the badge in `useWorkspaceData.ts` call the
  bridge when it is present and the browser API otherwise. Detecting the bridge is the only
  difference in the frontend.
- Closing the window keeps the app in the tray with the socket open, so notifications keep
  arriving; *Quit* is in the tray menu. Clicking a notification opens its conversation.
- *Open at login* is an app setting.

## Acceptance

- [ ] A mention in a background server raises a native notification, and clicking it opens
  the message.
- [ ] The dock or taskbar badge and the tray show the unread count across servers.
- [ ] A page from an origin the person did not add cannot call any command.

## Tests

Frontend: the bridge is used when present and the browser API otherwise. Rust: the
capability configuration refuses an unknown origin.
