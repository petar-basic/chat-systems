# CS-035 — Web Push for closed-app delivery

**Wave:** 9 — Product parity
**Area:** backend/api · frontend
**Blocked by:** ~~CS-004~~ ✅ shipped as `chat-worker`, CS-026 (badge counts)
**Blocks:** CS-038
**Roadmap:** existing item

## Problem

Notifications only arrive while a window is open. Close the tab or quit the browser and
mentions are invisible until the app is opened again.

For a Slack replacement this is not a polish item — it is the difference between an alert
system and a website you have to remember to check. It is also a prerequisite for the
mobile story (CS-038): a PWA on a phone is only useful if it can wake up.

## Approach

1. **Service worker with a `push` handler.** The app already ships as an installable PWA;
   add the push and `notificationclick` handlers to the existing worker rather than
   registering a second one.
2. **VAPID keys as instance config** (`VAPID_PUBLIC_KEY`, `VAPID_PRIVATE_KEY`,
   `VAPID_SUBJECT`), generated once and documented in `.env.example` and `RUNBOOK.md`.
   Rotating them invalidates every subscription — say so where the key is documented.
3. **Subscription storage:**
   ```sql
   CREATE TABLE push_subscriptions (
       id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
       endpoint    TEXT NOT NULL UNIQUE,
       p256dh      TEXT NOT NULL,
       auth        TEXT NOT NULL,
       user_agent  VARCHAR(255),
       created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
       last_used_at TIMESTAMPTZ
   );
   ```
   One row per browser or device. Prune on `410 Gone` from the push service — that is the
   only reliable signal a subscription is dead.
4. **Send from the worker**, from the same notification consumer that writes the rows
   today, using the `web-push` crate. Payloads are encrypted client-side by the spec, but
   keep them minimal anyway — sender, channel and a truncated preview, never the full
   message. A push payload passes through a third-party push service.
5. **Respect the preferences that already exist.** DND
   ([`user_dnd`](../../backend/migrations/20240305000010_user_dnd.sql)) and per-channel
   mute ([`channel_mute`](../../backend/migrations/20240305000009_channel_mute.sql)) must be
   evaluated before sending. A push that ignores DND is worse than no push.
6. **Do not double-notify.** If the user has a live WebSocket for that workspace, the
   in-app notification already fired — skip the push. The gateway knows who is connected;
   expose it to the worker through the presence index from CS-027.
7. **Badge count on the notification** uses the unread counts from CS-026, so the app icon
   badge and the push agree.
8. **`notificationclick` deep-links** to the message, reusing the existing permalink route.

## Acceptance

- [ ] A mention delivers a system notification with the app closed.
- [ ] DND and channel mute suppress it.
- [ ] Users with a live socket get the in-app notification only.
- [ ] Dead subscriptions are pruned on `410`.
- [ ] Payloads carry no full message content.
- [ ] Clicking opens the app at the right message.

## Tests

Worker tests against a mock push service: assert one request per live subscription, none
under DND or mute, none when the user is connected, and pruning on `410`. A Playwright
spec that grants notification permission, registers, and asserts the subscription round
trip — end-to-end delivery itself needs a real push service and stays manual, documented in
`docs/manual-qa.md`.
