# CS-143 — Signing in from a client the server does not serve

**Wave:** 37 — mobile apps (2026-11-04, renumbered from 33 on 2026-11-13), released as 5.0.0
**Area:** backend · frontend · mobile
**Blocked by:** CS-141
**Blocks:** CS-144

## Problem

The web client signs in with cookies on the server's own origin, and the desktop app
works because it shows the server's own page. A phone app's client is bundled into the app
and runs on the app's origin (`tauri://localhost`, `http://tauri.localhost`), where the
server's cookies are not sent, and its requests are cross-origin.

## Approach

- The bundled client signs in with tokens: `/auth/login` and `/auth/refresh` already return
  and accept them (`AuthSession`). The refresh token is kept in the Keychain or the
  Keystore through a Tauri plugin, never in the page's storage; the access token lives in
  memory.
- The server accepts the app's origins in CORS, as config, and only for the routes the app
  uses. A request from the app with a cookie is refused, so a token and a cookie are never
  mixed.
- SSO, passkeys (CS-118) and the emailed code (CS-119) come back into the app through its
  deep link, using the single-use exchange code desktop SSO uses (`/auth/oidc/exchange`),
  widened from SSO to every way of signing in.
- Signing out, and an admin revoking the session, clear the stored token.

## Acceptance

- [ ] The bundled client signs in to a server by password, SSO, passkey and emailed code,
  and stays signed in across a restart.
- [ ] Nothing about the session is in the page's storage.
- [ ] A revoked session signs the app out at its next request.
- [ ] The web client still signs in with cookies, unchanged.

## Tests

Backend: CORS for the app origins and not others, cookie and token not mixed, the exchange
code for each way of signing in. Frontend: the token client beside the cookie client.
