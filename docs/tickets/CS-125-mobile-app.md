# CS-125 — Phone apps for iOS and Android

**Wave:** 33 — mobile apps (2026-11-04), released as 5.0.0
**Area:** mobile · frontend · ops
**Blocked by:** CS-122 · CS-123 · CS-124
**Blocks:** CS-126

## Problem

On a phone the product is a PWA: no App Store or Play Store listing, push only where the
browser allows it, and on iOS a home-screen app that Safari decides how long to keep.

## Approach

- The desktop project gains iOS and Android targets (Tauri 2), carrying a build of the web
  client, which already has a phone layout tested in E2E.
- *Add a server* and a list of servers as on desktop, each server's workspaces in the
  drawer instead of a rail.
- Deep links (`chat-systems://`) open a server, a channel or a message, and finish sign-in
  (CS-124).
- Notifications come through CS-126 once it ships; until then the app notifies while it is
  open, as the PWA does.
- The release workflow builds an `.ipa` for TestFlight and an `.aab` for Play's internal
  track, signed with the project's accounts. Store listing is a separate step the
  maintainer takes.

## Acceptance

- [ ] On an iPhone and an Android phone the app adds a server, signs in, reads and sends
  messages, uploads a photo and joins a huddle.
- [ ] A link to a message opens it in the app.
- [ ] A tagged release produces both builds.

## Tests

E2E in the phone viewport, as today. On devices, the checklist in `docs/manual-qa.md`,
which this ticket extends with a phone section.
