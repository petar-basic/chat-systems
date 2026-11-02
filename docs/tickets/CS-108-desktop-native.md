# CS-108 — One rail for servers and workspaces, notifications, badge and tray

**Wave:** 29 — a desktop app (2026-11-02)
**Area:** desktop · frontend
**Blocked by:** —
**Blocks:** —

## Problem

- **Two rails.** CS-107 puts a rail of servers on the left, and each server's web app shows its
  own rail of workspaces beside it, with its own *Add instance*, which inside the app
  cannot reach another server anyway. A person thinks in workspaces, not servers.
- **A quiet app.** Inside a web view, `Notification` and `navigator.setAppBadge` are missing
  or do nothing, so the app would be quieter than the browser tab it replaces.

## Approach

- **The bridge.** The app exposes two commands to the page: `report_workspaces` and `notify`.
  Tauri capabilities allow them only from the origins the person added, and nothing else is
  exposed: no file system, no shell. The Dock badge is the sum of the reported unread counts,
  and links to other sites already open in the system browser, so neither needs a command.
- **One rail.** Each server's web app reports its workspaces, their icons and unread counts,
  and the app's rail shows them grouped by server, as the web app's own rail groups them by
  instance. Clicking a workspace shows that server and opens the workspace in it. Inside the
  app the web app hides its own rail and *Add instance*; the rail's *+* adds a server.
- `frontend/src/lib/notifications.ts` calls the bridge when it is present and the browser
  API otherwise. Detecting the bridge is the only difference in the frontend.
- Closing the window keeps the app in the tray with the socket open, so notifications keep
  arriving; *Quit* is in the tray menu. Clicking a notification opens its conversation.
- *Open at login* is an app setting.

## Acceptance

- [x] One rail shows every server's workspaces with their unread counts, and the web app's
  own rail and *Add instance* are gone inside the app.
- [x] A mention in a background server raises a native notification, and clicking it opens
  the message.
- [ ] The dock or taskbar badge and the tray show the unread count across servers.
- [ ] A page from an origin the person did not add cannot call any command.

## Tests

Frontend: the bridge is used when present and the browser API otherwise. Rust: the
capability configuration refuses an unknown origin.
