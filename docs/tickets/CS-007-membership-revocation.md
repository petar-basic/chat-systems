# CS-007 — Propagate membership removal to live sockets

**Wave:** 1 — Access control
**Area:** backend/api · realtime
**Blocked by:** ~~CS-003~~ ✅ shipped as `sessions.rs`, ~~CS-002~~ ✅ shipped as `authz.rs`
**Blocks:** CS-033
**Audit finding:** S2 (HIGH), S5 (MEDIUM-HIGH)

## Problem

Two related gaps.

**Removal does not reach the socket.**
[`remove_channel_member`](../../backend/api/src/workspace/routes.rs#L593) deletes the
`channel_members` row and publishes nothing.
[`remove_member`](../../backend/api/src/workspace/routes.rs#L238) does the same for
`workspace_members`. The realtime gateway checks membership only at `channel.join`
([`ws_handler.rs:222`](../../backend/realtime/src/ws_handler.rs#L222)); after that the
channel id sits in the connection's in-memory `subscribed_channels` and is never
re-checked.

So removing somebody from a private channel does not stop them reading it. They keep
receiving every message until the socket closes — bounded by the access-token deadline
([`ws_handler.rs:113`](../../backend/realtime/src/ws_handler.rs#L113)), which is
`ACCESS_TOKEN_EXPIRY`, default one hour.

**Workspace removal leaves channel membership behind.**
[`WorkspaceRepo::remove_member`](../../backend/api/src/workspace/repo.rs#L212) deletes
only from `workspace_members`. There is no cascade to `channel_members` — the foreign
keys point at `channels` and `users`, not at the membership row. HTTP access is still
denied, because `authz::require_channel_access` checks workspace membership first. But
the rows persist, which means:

- the realtime gateway, which checks `channel_members` alone, still says yes;
- re-adding the person months later silently restores every private channel they were
  ever in, with nobody re-inviting them.

## Approach

Removal becomes an event, and the gateway acts on it.

1. **Publish on removal.** Both handlers publish through the workspace publisher after a
   successful delete:
   ```
   channel.member_removed   { channel_id, user_id, workspace_id }
   workspace.member_removed { workspace_id, user_id }
   ```
   Add `backend/api/src/workspace/publisher.rs` if the feature has none — the conventions
   put event publishing there, not inline in routes.
2. **Cascade in one transaction.** `remove_member` deletes the `workspace_members` row
   and every `channel_members` row for that user in that workspace, in one transaction:
   ```sql
   DELETE FROM channel_members cm
    USING channels c
    WHERE cm.channel_id = c.id
      AND c.workspace_id = $1
      AND cm.user_id = $2
   ```
   Follow the existing `*_tx` naming used by `add_member_tx` / `claim_invite_use_tx`.
3. **Gateway drops the subscription.** New arms in
   [`event_consumer.rs`](../../backend/realtime/src/event_consumer.rs):
   - `channel.member_removed` → for every connection of that user, `leave_channel`, then
     send `{"type":"channel.access_revoked","channel_id":…}` so the client can close the
     view and drop the query cache instead of showing a frozen channel.
   - `workspace.member_removed` → leave every channel of that workspace and unsubscribe
     the workspace.
   `ConnectionManager` needs `leave_channel_for_user(user_id, channel_id)` and
   `leave_workspace_for_user(user_id, workspace_id)`; both iterate `user_connections` and
   reuse the existing per-connection `leave_channel`.
4. **Do not use `sessions::revoke` here.** Removing somebody from one channel must not
   log them out of the instance. That is why this ticket needs its own events rather than
   reusing `sessions::revoke`. It does rely on the close-frame and client-reconnect
   behaviour introduced there.
5. **Frontend.** `wsQuerySync` handles `channel.access_revoked` by invalidating the
   channel list, removing the channel's cached messages, and navigating away if it is the
   open channel. Query keys go through the `QUERY_KEYS` factory as usual.

## Acceptance

- [ ] Removing a user from a private channel stops message delivery to their live socket
      within one event round trip, not one token lifetime.
- [ ] Removing a user from a workspace deletes their `channel_members` rows in the same
      transaction.
- [ ] Re-adding a previously removed user grants no private channels.
- [ ] The client leaves the revoked channel view instead of silently freezing.
- [ ] Leaving voluntarily (`remove_channel_member` where `auth.user_id == user_id`) uses
      the same path and does not regress.

## Tests

Realtime tests: subscribe a connection to a channel, publish `channel.member_removed`,
assert a subsequent broadcast is not delivered and the revoke notice is.
`http_tests/workspace.rs`: remove a member, assert no `channel_members` rows remain for
that workspace, re-add, assert they are in no private channel. An E2E spec covering
kick-from-private-channel is the regression test that would have caught this.
