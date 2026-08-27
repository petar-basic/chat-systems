# CS-045 — Email a mention that reached nobody

**Wave:** 10 — Guest containment and operational readiness
**Area:** backend/api (worker)
**Blocked by:** ~~CS-035~~ ✅ shipped (Web Push)
**Blocks:** —
**Audit finding:** MEDIUM — notification gap

## Problem

Email is used for exactly two things in this product: invitations and password resets
([`auth/service.rs`](../../backend/api/src/auth/service.rs#L558)). Nothing else is ever
sent.

So a person who is mentioned while offline, and who has not granted push permission or has
no registered browser, learns about it the next time they open the app — which for somebody
on holiday, or somebody who only opens it in the morning, is not a notification system.
Slack sends an email; every team migrating from it expects that behaviour and will read its
absence as messages being lost.

CS-035 covers the case where a browser is registered. This ticket covers the case where
none is, which is the default for anybody who clicked "no" once.

## Approach

1. **Send only when nothing else could have.** The worker already knows, on the notification
   path, whether the person has a live socket (CS-027 presence) and whether they have any
   push subscription rows. Email is the third branch: no socket, no subscription, not muted,
   not in do-not-disturb. Anything else and it stays silent — a duplicate email for a
   message somebody already read is how people build a filter rule for your domain.
2. **Digest, not one per mention.** A short delay before sending (default 5 minutes) and one
   email per person per workspace covering what accumulated, so a thread that mentions
   somebody four times is one email. The delay is also a second chance for them to come
   online, at which point it is dropped.
3. **Carry less than the push payload does.** Who mentioned them, in which channel, and a
   link — no message text. Mail sits in an inbox on somebody else's server, indefinitely,
   and is the least private transport this product touches.
4. **Reuse the existing SMTP path** and its configuration; do not introduce a second mail
   client. If SMTP is not configured, this feature is simply off, exactly like Web Push is
   off without VAPID keys.
5. **A per-user switch, defaulting on.** Somebody who has push does not need this, and the
   preference belongs beside the existing notification settings rather than in a new panel.

## Acceptance

- [ ] A mention for somebody with no socket and no push subscription produces one email.
- [ ] Somebody with a live socket, an active push subscription, DND, or a muted channel gets
      none.
- [ ] Several mentions inside the digest window produce one email, not several.
- [ ] Coming online inside the window cancels it.
- [ ] The email carries no message body.
- [ ] With SMTP unconfigured, nothing is attempted and nothing is logged as an error.

## Tests

Worker tests against MailHog, the way the invite flow is already tested: assert one message
for the unreachable case and none for each of the four suppressed ones, assert the digest
collapses repeats, and assert the body contains the channel and the sender but not the text
that was posted.
