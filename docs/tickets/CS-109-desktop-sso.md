# CS-109 — SSO that finishes in the app

**Wave:** 29 — a desktop app (2026-11-02)
**Area:** desktop · backend
**Blocked by:** CS-107
**Blocks:** —

## Problem

*Sign in with SSO* sends the window to the identity provider. CS-107 opens other origins in
the system browser, and several providers (Google among them) refuse to sign in inside an
embedded web view anyway, so the session ends up in the browser instead of the app.

## Approach

- The app registers a `chat-systems://` URL scheme.
- `/auth/oidc/start` accepts a `client=desktop` flag. Instead of setting cookies, the
  callback hands the browser a short-lived, single-use code and redirects it to
  `chat-systems://sso?server=…&code=…`.
- The app exchanges the code at a new `POST /auth/oidc/exchange` from its server's window,
  which sets the session cookies there, as a password sign-in does.
- The code lives a minute, is bound to the server it came from, and works once.

## Acceptance

- [ ] SSO from the app opens the system browser and lands back in the app signed in.
- [ ] A code works once, only within its minute, and only on the server that issued it.

## Tests

API: the exchange accepts a fresh code once and refuses a reused, expired or foreign one.
