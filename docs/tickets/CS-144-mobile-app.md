# CS-144 — Phone apps for iOS and Android

**Wave:** 37 — mobile apps (2026-11-04, renumbered from 33 on 2026-11-13), released as 5.0.0
**Area:** mobile · frontend · ops
**Blocked by:** CS-141 · CS-142 · CS-143
**Blocks:** CS-145

## Problem

On a phone the product is a PWA: no App Store or Play Store listing, push only where the
browser allows it, and on iOS a home-screen app that Safari decides how long to keep.

## Approach

- The desktop project gains iOS and Android targets (Tauri 2), carrying a build of the web
  client, which already has a phone layout tested in E2E.
- *Add a server* and a list of servers as on desktop, each server's workspaces in the
  drawer instead of a rail.
- Deep links (`chat-systems://`) open a server, a channel or a message, and finish sign-in.
- Signing in with tokens, as CS-143 made the server and the web client do it, with what CS-143
  left to the app: the refresh token in the Keychain or the Keystore through a Tauri plugin,
  never in the page's storage, and the access token in memory. SSO, passkeys (CS-118) and the
  emailed code (CS-119) come back into the app through its deep link, using the single-use
  exchange code (`/auth/oidc/exchange`, which now returns the session), widened from SSO to
  every way of signing in. Signing out, and an admin revoking the session, clear the stored
  token.
- Notifications come through CS-145 once it ships; until then the app notifies while it is
  open. The webviews have no `Notification`, so it goes through the app's plugin, as on
  desktop (CS-108).
- What CS-142 found the app must handle: the Android WebView cannot share the screen, and
  Android 16 draws under the status bar unless the app passes the insets in.
- The release workflow builds an `.ipa` for TestFlight and an `.aab` for Play's internal
  track, signed with the project's accounts. Store listing is a separate step the
  maintainer takes.

## Acceptance

- [ ] On an iPhone and an Android phone the app adds a server, signs in, reads and sends
  messages, uploads a photo and joins a huddle.
- [ ] The app signs in to a server by password, SSO, passkey and emailed code, and stays
  signed in across a restart.
- [ ] Nothing about the session is in the page's storage.
- [ ] A revoked session signs the app out at its next request.
- [ ] On both phones a huddle is held for ten minutes with the screen locked, both ways.
  CS-142 left this open: the iOS simulator cut the microphone at the lock and the iPhone
  did not in its first minute. Whatever fails is written down as a known limitation.
- [ ] A link to a message opens it in the app.
- [ ] A tagged release produces both builds.

## Tests

E2E in the phone viewport, as today. On devices, the checklist in `docs/manual-qa.md`,
which this ticket extends with a phone section.
