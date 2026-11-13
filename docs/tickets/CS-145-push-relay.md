# CS-145 — A push relay for phones

**Wave:** 37 — mobile apps (2026-11-04, renumbered from 33 on 2026-11-13), released as 5.0.0
**Area:** backend · mobile · ops
**Blocked by:** CS-144, and the decision on who runs the relay
**Blocks:** —

## Problem

APNs and FCM accept a push only from the publisher of the app, so a self-hosted server
cannot reach a phone running the app the project publishes. Without push, a phone app
learns of a message only while it is open.

## Approach

- A small relay service, run by the publisher, that holds the APNs and FCM credentials and
  forwards a push it is given. It sees a device token and an encrypted payload, never a
  message: the phone generates a key on registration and gives it to its own server only,
  and the server encrypts the notification with it.
- A server that wants push for its users is configured with the relay's URL; with none, the
  app notifies only while it is open.
- The relay rate-limits per server, drops tokens APNs or FCM report as dead and keeps no
  log of payloads.
- The decision this waits on: whether the project runs a relay, and where. It is written
  down in [native-apps-plan.md](../native-apps-plan.md) under *Open decisions*.

## Acceptance

- [ ] A mention reaches a phone with the app closed, through the relay, and the relay's
  logs show no message text.
- [ ] A server with no relay configured still works, without push.

## Tests

Backend: the encrypted payload and the token lifecycle. The relay: forwarding, the rate
limit, dead tokens.
