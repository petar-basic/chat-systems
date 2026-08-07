# CS-027 — Presence without a Redis keyspace scan

**Wave:** 6 — Performance
**Area:** realtime
**Blocked by:** —
**Blocks:** —
**Audit finding:** A2 (MEDIUM)

## Problem

Presence is stored as one Redis key per user per node,
`presence:{user_id}:{node_id}` with a 60-second TTL
([`connection_manager.rs:271`](../../backend/realtime/src/connection_manager.rs#L271)).
Reading it means finding those keys, and the only way to find keys by pattern is to scan:

```rust
let keys = Self::scan_keys(&mut conn, "presence:*").await
```

[`get_online_users`](../../backend/realtime/src/connection_manager.rs#L325) walks the
**entire Redis keyspace** in `COUNT 100` batches. It is called by
`online_users_in_workspace`, which runs on every `subscribe` frame — that is, on every
WebSocket connection.

The same Redis also holds rate-limit keys (one per user per window, growing with CS-012's
per-class keys), revocation flags and huddle membership. So the scan cost grows with
unrelated traffic. After a deploy, 70 users reconnecting means ~200 full keyspace scans in
a few seconds.

It works at this size. It is the wrong shape, and it degrades from a direction nobody will
be looking at.

## Approach

Index presence instead of scanning for it. The TTL semantics that motivated per-key
expiry have to be preserved — a node that dies must not leave users online forever.

1. **A sorted set per workspace, scored by expiry timestamp:**
   ```
   ZADD presence:ws:{workspace_id} {now + PRESENCE_TTL} {user_id}
   ```
   Online users are `ZRANGEBYSCORE presence:ws:{ws} {now} +inf` — one call, bounded by the
   workspace's member count instead of the keyspace. Stale entries are removed lazily with
   `ZREMRANGEBYSCORE ... -inf {now}` on read, so a dead node self-heals without a
   background sweeper.
2. **Heartbeat refreshes the score**, which the loop already does every 30 seconds
   ([`ws_handler.rs:125`](../../backend/realtime/src/ws_handler.rs#L125)) — the call
   becomes a `ZADD` instead of a `SET EX`.
3. **A user is in several workspaces.** `user_workspace_ids` already exists
   ([`connection_manager.rs:371`](../../backend/realtime/src/connection_manager.rs#L371));
   cache it per connection at subscribe time rather than querying on every heartbeat, and
   refresh it when a `workspace.member_removed` event arrives (CS-007).
4. **Multi-node correctness.** The node id currently disambiguates two nodes holding the
   same user. With a sorted set, the last writer wins and the score is a max — which is the
   correct semantics: the user is online until the latest heartbeat expires, regardless of
   which node saw them. Keep `presence_clear` publishing offline only when the local node
   held the last connection, exactly as `cleanup` does today.
5. **Remove `scan_keys` entirely** once nothing calls it, so it cannot come back.
6. **Metric:** `realtime_presence_query_duration_seconds`, so the improvement is visible
   and a future regression is not silent.

## Acceptance

- [ ] No `SCAN` remains in the realtime crate.
- [ ] Presence for a workspace is one Redis call bounded by member count.
- [ ] A killed node's users go offline within the TTL without a sweeper.
- [ ] Multi-tab and multi-node presence behaviour is unchanged.
- [ ] Presence query duration is exported.

## Tests

Realtime tests: two connections for one user across simulated nodes, assert online while
either lives and offline after both expire. Assert stale entries are trimmed on read.
Assert the workspace-id cache is invalidated by `workspace.member_removed`.
