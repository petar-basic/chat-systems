# CS-107 — A desktop app that opens any server

**Wave:** 29 — a desktop app (2026-11-02)
**Area:** desktop
**Blocked by:** CS-106
**Blocks:** CS-108, CS-109, CS-110

## Problem

There is no desktop app. A team that lives in chat wants it in the dock or taskbar, opening
at login, rather than a browser tab among thirty.

## Approach

- A `desktop/` Tauri 2 project in this repository, its own Cargo workspace so the backend's
  build and CI are untouched.
- First start asks for a server URL, checks it with `GET /api/instance/info`, and opens that
  server's web app in a window. Servers are kept in the app's own config; a sidebar or menu
  switches between them and adds or removes one.
- A window only ever shows its server's origin. A link elsewhere opens the system browser;
  downloads go to the system's Downloads folder.
- Window size and position, and the last server, are restored on start.

## Acceptance

- [ ] A person can add two servers, sign in to each and switch between them.
- [ ] Nothing outside a server's origin opens inside the app.
- [ ] A server running any 3.x or 4.x release works, because the app shows its own web app.

## Tests

Rust unit tests for URL normalisation and origin checks. The app's own E2E is out of scope;
the web app it shows is covered by the existing suite.
