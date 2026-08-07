# CS-010 — Guest scoping in message search

**Wave:** 1 — Access control
**Area:** backend/api
**Blocked by:** ~~CS-002~~ ✅ shipped as `authz.rs`
**Blocks:** —
**Audit finding:** S7 (MEDIUM)

## Problem

`require_channel_access` deliberately treats guests more strictly than members — a guest
must be an explicit channel member even for a public channel
([`messaging/routes.rs:497`](../../backend/api/src/messaging/routes.rs#L497)):

```rust
if member.role == WorkspaceRole::Guest
    || channel.channel_type == ChannelType::Private
    || channel.channel_type == ChannelType::GroupDm
{
    // must be in channel_members
}
```

[`MessageRepo::search`](../../backend/api/src/messaging/repo.rs#L286) re-derives
visibility in SQL and drops the guest clause:

```sql
AND ( c.channel_type = 'public'
      OR EXISTS (SELECT 1 FROM channel_members cm
                  WHERE cm.channel_id = c.id AND cm.user_id = $3) )
```

A guest — an external contractor or client account — can therefore read the content of
every public channel through `/api/search` that the same account is refused through
`/channels/:id/messages`. Same data, two answers, depending on which endpoint you ask.

This is the drift `authz` was built to stop: a rule expressed once in
Rust and once again in SQL, and the copies drifted.

## Approach

Derive the SQL predicate from the same decision the Rust helper makes, instead of
restating it.

1. **`search_messages` resolves the caller's role first.** It already calls
   `is_workspace_member`; replace that with `authz::require_workspace_member`, which
   returns the `WorkspaceMember` including the role.
2. **Pass the role into the query** as part of the existing `MessageSearch` struct:
   ```rust
   pub struct MessageSearch<'a> {
       pub query: &'a str,
       pub workspace_id: Uuid,
       pub requester_id: Uuid,
       pub requester_is_guest: bool,
       pub channel_id: Option<Uuid>,
       pub author_id: Option<Uuid>,
       pub limit: i64,
       pub offset: i64,
   }
   ```
3. **One visibility clause that mirrors the helper**, guest handling included:
   ```sql
   AND (
     EXISTS (SELECT 1 FROM channel_members cm
              WHERE cm.channel_id = c.id AND cm.user_id = $3)
     OR (NOT $8 AND c.channel_type = 'public')
   )
   ```
   Explicit membership always grants; the public shortcut applies only to non-guests.
   `GroupDm` and `private` already fall out correctly because neither is `'public'`.
4. **Keep it one query.** The alternative — fetch candidates then filter in Rust — breaks
   `LIMIT`/`OFFSET` and `ts_rank` ordering. The predicate belongs in SQL; what must not
   be duplicated is the *rule*, which is now a single boolean handed down from `authz`.
5. **Add a doc line to `docs/backend.md`** under the search endpoint stating that search
   visibility equals `require_channel_access` visibility, so the next person extending
   either one knows they are a pair.

## Acceptance

- [ ] A guest searching finds results only from channels they are an explicit member of.
- [ ] A member's search results are unchanged.
- [ ] Private and group-DM channels remain membership-gated for everyone.
- [ ] The `channel_id` filter still runs `authz::require_channel_access` first, so a
      guest scoping to a forbidden channel gets 403 rather than an empty list.

## Tests

`http_tests/messaging.rs`: seed a public channel with a matching message, add a guest to
the workspace but not the channel, search → empty. Add the guest to the channel → the
message appears. Same seed with a `member` role → appears without channel membership.
Guest scoping to a non-member channel → 403.
