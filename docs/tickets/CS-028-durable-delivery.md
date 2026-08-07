# CS-028 — Durable realtime delivery (Redis Streams)

**Wave:** 7 — Reliability
**Area:** realtime · frontend
**Blocked by:** CS-004 (same transport layer), CS-014, CS-027
**Blocks:** —
**Roadmap:** existing item, unchanged in substance

## Problem

Events fan out over Redis pub/sub. A client that is disconnected when an event is
published never receives it — pub/sub has no backlog. Recovery is a refetch of open views
on reconnect ([`realtimeBackfill.ts`](../../frontend/src/lib/realtimeBackfill.ts)), so
nothing is permanently lost from the database, but delivery over the socket is
**at-most-once** and there is no gap replay.

Two visible consequences:

- Views that are not open are not refetched, so unread state and badges can be stale until
  something else triggers a fetch.
- A slow client is dropped on backpressure
  ([`connection_manager.rs:186`](../../backend/realtime/src/connection_manager.rs#L186),
  surfaced as `realtime_backpressure_drops_total`) and silently loses whatever was in
  flight. It also waits for the next heartbeat to notice, because no close frame is sent.

## Why last in the reliability sequence

It reworks the transport that CS-004 splits, CS-007 publishes onto and CS-027 uses. Doing
it before those means doing it twice; doing it after means each of them lands on a
transport that is not moving underneath it.

## Approach

1. **One stream per workspace.** Publishers `XADD events:{workspace_id} * payload {json}`
   instead of `PUBLISH`. The stream id is the sequence number the whole design hangs on.
2. **Client tracks its position.** The gateway sends the stream id with every event; the
   client persists the last processed id per workspace and sends it in the `subscribe`
   frame on reconnect. The gateway replays with `XRANGE` from that id, then joins the live
   tail with `XREAD BLOCK`. Cap the replay window — beyond N events or T minutes, tell the
   client to do a full refetch instead, which is the existing backfill path and remains the
   correct answer for a long absence.
3. **Idempotency on the client.** Delivery becomes at-least-once, so the client must
   tolerate duplicates. Message ids are already unique and the Query cache reconciles by
   id; audit `wsQuerySync` for handlers that append rather than upsert — reactions,
   typing and presence in particular.
4. **Trim aggressively.** `XADD ... MAXLEN ~ 10000` per workspace, plus an age-based trim in
   the worker. The stream is a replay buffer, not storage; the database is the source of
   truth.
5. **Fix the backpressure path in the same change.** Send a WebSocket close frame with a
   reason before dropping a slow client so it reconnects immediately and replays from its
   last id, rather than waiting up to 30 seconds and then refetching everything.
6. **Authorization is still per-event, not per-stream.** A workspace stream carries events
   for channels a given connection may not see. The gateway must apply the same
   subscription filter on replay that it applies live — this is the part where a naive
   implementation leaks private channels into a replay. Route replayed events through
   exactly the same `fan_out` predicate, and after CS-007 the connection's subscription set
   is already authoritative.
7. **Consumer groups for the worker.** With `XREADGROUP`, the notification and hook
   consumers get at-least-once delivery with acknowledgement, which also removes the
   single-replica constraint CS-004 accepted as a temporary answer. Do this in the same
   ticket — it is the payoff that makes the transport change worth it.

## Acceptance

- [ ] A client disconnected for under the replay window receives every missed event on
      reconnect, exactly once after client-side deduplication.
- [ ] Beyond the window the client falls back to full refetch.
- [ ] Replayed events respect channel visibility identically to live events.
- [ ] Backpressure drops send a close frame and the client reconnects immediately.
- [ ] Worker consumers use consumer groups and can run more than one replica.
- [ ] Streams are trimmed by both length and age.

## Tests

Realtime tests: publish while a connection is down, reconnect with a stored id, assert the
gap is replayed in order and nothing outside the subscription set appears. Assert a client
past the window is told to refetch. Assert two worker replicas process each event once.
An E2E spec that restarts the gateway mid-conversation already exists — extend it to
assert no missed message rather than only successful reconnection.
