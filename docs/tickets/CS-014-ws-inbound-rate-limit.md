# CS-014 — WebSocket inbound message rate limiting

**Wave:** 2 — Abuse and resource limits
**Area:** realtime
**Blocked by:** CS-007 (adds arms to the same handler)
**Blocks:** —
**Audit finding:** S10 (MEDIUM)

## Problem

[`handle_client_message`](../../backend/realtime/src/ws_handler.rs#L170) has no
throughput limit. Every inbound frame is parsed and dispatched, and most arms hit the
database:

- `subscribe` → `is_workspace_member` (one query)
- `channel.join`, `typing.start`, `typing.stop` → `is_channel_member` (one query each)
- huddle arms → one or two Redis round trips each

A single authenticated client can loop `typing.start` as fast as the socket accepts
frames and saturate the Postgres pool (`PG_POOL_MAX`, default 20) shared by the whole
gateway. Nothing in the loop yields to a limiter, and `typing.*` in particular has no
legitimate reason to exceed a few per second.

The HTTP side is limited per user; the WebSocket side is the same trust boundary with no
equivalent.

## Approach

Limit per connection, in memory, before any I/O.

1. **Token bucket on the `Connection`.** A per-connection counter is enough — the socket
   is already the unit of abuse, and keeping it local avoids a Redis call to defend
   against a flood of Redis calls:
   ```rust
   struct InboundBudget {
       tokens: f32,
       last_refill: Instant,
   }
   ```
   Refill at `INBOUND_MSGS_PER_SEC` (default 20) with a burst of 40. Charge one token per
   frame before parsing.
2. **Cache membership decisions per connection.** The bigger win than the limiter: a
   connection asks "is this user in this channel" repeatedly for the same channel. Keep a
   small per-connection map of resolved channel ids with a short TTL (30s) and consult it
   before querying. CS-007 already invalidates on removal, so the cache cannot go stale
   in the dangerous direction — wire the same events to clear it.
3. **Coalesce typing events.** `typing.start` for a channel the connection already
   started typing in within the last few seconds is a no-op — drop it before the
   membership check. This alone removes most of the legitimate traffic that the limiter
   would otherwise have to absorb.
4. **Over budget → close, do not silently drop.** Send a close frame with a reason and
   drop the connection, matching how backpressure is handled in
   [`connection_manager.rs:186`](../../backend/realtime/src/connection_manager.rs#L186).
   A client that is silently ignored retries harder.
5. **Cap the frame size.** Configure the axum `WebSocketUpgrade` with
   `max_message_size` and `max_frame_size` (64 KiB is generous for control frames) so a
   single huge frame cannot be buffered.
6. **Metrics:** `realtime_inbound_dropped_total` and
   `realtime_membership_cache_hits_total`, both listed in `RUNBOOK.md`.

## Acceptance

- [ ] A client exceeding the inbound budget is closed with a reason.
- [ ] Repeated `channel.join` for the same channel issues at most one query per TTL.
- [ ] Redundant `typing.start` frames do not reach the database.
- [ ] Frame size is capped.
- [ ] Normal two-browser E2E traffic never trips the limiter.

## Tests

Realtime tests: drive a connection past the budget and assert it is closed; assert a
second `channel.join` for the same channel does not query. Confirm the membership cache
is cleared by `channel.member_removed` from CS-007 — that assertion is what keeps the
cache from reintroducing the bug CS-007 fixes.
