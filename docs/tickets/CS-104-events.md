# CS-104 — Events for apps, over HTTP or a socket

**Wave:** 28 — what 4.0.0 needs (2026-10-29 feedback on 3.7.0)
**Area:** backend · realtime · docs
**Blocked by:** CS-103
**Blocks:** CS-105

## Problem

A bot from CS-103 can act but cannot listen. The only way to hear about a message today is an
outgoing webhook, which is bound to channels by an admin, carries only new messages and
retries without telling the app it is a retry. A bot that answers mentions, watches reactions
or welcomes new members has nothing to subscribe to, and an app behind a firewall cannot be
reached by any of it.

## Approach

- **Subscriptions** on the app: `message` (in channels the bot is in), `app_mention`,
  `reaction_added`/`removed`, `member_joined_channel`, `channel_created`/`archived`. Each
  needs the matching read scope.
- **One envelope**: `{ event_id, event_time, type, workspace_id, event }`, at least once,
  in order per channel.
- **Over HTTP**: a signed POST to the app's request URL (the outgoing-hook signature and SSRF
  check), verified once with a challenge when the URL is saved. Delivery runs from the
  worker on the workspace streams; a failure is retried with backoff for up to an hour with
  `X-Chat-Retry-Num`, after which the app is shown as failing in *Apps*.
- **Over a socket (socket mode)**: an app-level token (`capp-…`, scope `connections:write`)
  calls `POST /apps/connections.open` for a short-lived URL on the realtime gateway; events
  arrive as envelopes with an `envelope_id` the app must ack within three seconds or they are
  redelivered. Slash commands are delivered the same way, so an app with no public URL can
  have commands. Several sockets per app share the load.
- `docs/bot-api.md` gains events and socket mode; the example bot gets a socket-mode variant.

## Acceptance

- [ ] A subscribed app receives each event once in the normal case, a retry is marked as one,
  and nothing is lost across a worker or gateway restart.
- [ ] An app with no public URL receives events and slash commands over socket mode.
- [ ] An app never hears about a channel it is not in.

## Tests

API: the challenge, each event type, scope and membership filtering, retries and the failing
state. Realtime: socket-mode auth, ack and redelivery, two sockets sharing an app, a revoked
token closing the socket. The socket-mode example bot answers a mention in CI.
