# CS-002 — Central authorization module

**Wave:** 0 — Safety net and structural groundwork
**Area:** backend/api
**Blocked by:** —
**Blocks:** CS-009, CS-010, CS-018, CS-020 — and every future feature

## Problem

Authorization logic is correct today but duplicated by copy. There are three separate
implementations of `require_workspace_member`:

- [`files/routes.rs:222`](../../backend/api/src/files/routes.rs#L222)
- [`conversations/routes.rs:41`](../../backend/api/src/conversations/routes.rs#L41)
- [`huddle/routes.rs:215`](../../backend/api/src/huddle/routes.rs#L215)

two of `require_channel_access` ([`messaging/routes.rs:479`](../../backend/api/src/messaging/routes.rs#L479),
[`huddle/routes.rs:183`](../../backend/api/src/huddle/routes.rs#L183)), and two variants of
the role gate (`workspace::routes::require_role`, `hooks::routes::require_ws_role`).

Across 95 routes, every handler has to remember to call the right one. Nothing in the
type system enforces it. The two access-control bugs in Wave 1 — CS-009 and CS-010 —
are both instances of the same failure: a new module reimplemented the check and left a
rule out.

## Why now

Every Wave 1 ticket edits one of these helpers. Consolidating first means each fix is
written once, in one place, and automatically applies everywhere. Consolidating later
means merging five divergent copies.

## Approach

Introduce `backend/api/src/authz.rs` as the single home for permission predicates. It is
a cross-cutting helper module, not a feature, so it sits at the crate root next to
`middleware.rs` and `rate_limit.rs`.

1. **One implementation per predicate**, each returning the domain object it proved
   access to, so callers do not re-fetch:
   ```rust
   pub async fn require_workspace_member(state: &AppState, ws_id: Uuid, user_id: Uuid)
       -> AppResult<WorkspaceMember>

   pub async fn require_workspace_role(state: &AppState, ws_id: Uuid, user_id: Uuid,
       minimum: WorkspaceRole) -> AppResult<WorkspaceMember>

   pub async fn require_channel_access(state: &AppState, ch_id: Uuid, user_id: Uuid)
       -> AppResult<ChannelAccess>

   pub async fn require_channel_moderator(state: &AppState, ch_id: Uuid, user_id: Uuid)
       -> AppResult<ChannelAccess>

   pub async fn require_conversation_participant(state: &AppState, conv_id: Uuid,
       user_id: Uuid) -> AppResult<Conversation>
   ```
2. **`ChannelAccess` carries the decision, not just the channel.** Today the guest rule
   lives inside `require_channel_access` and is invisible to callers, which is exactly
   why search re-derived it and got it wrong (CS-010):
   ```rust
   pub struct ChannelAccess {
       pub channel: Channel,
       pub member: WorkspaceMember,
       pub channel_member: Option<ChannelMember>,
   }

   impl ChannelAccess {
       pub fn is_guest(&self) -> bool
       pub fn requires_explicit_membership(&self) -> bool
   }
   ```
   Any query that needs to reproduce visibility (search, file access, unread) asks this
   type instead of rewriting the predicate in SQL.
3. **The rules stay exactly as they are today.** This ticket is a pure move plus
   deduplication. Behaviour changes belong to CS-009 and CS-010 so that a regression is
   attributable to one commit.
4. **Delete the per-module copies** and re-export nothing — callers use
   `crate::authz::require_*` explicitly, so a `grep authz::` shows every authorized
   handler at a glance.
5. Repos keep their existing `get_member` / `get_channel_member` methods. `authz` calls
   them; it does not issue SQL of its own. Layer boundaries are unchanged.

## Acceptance

- [ ] `backend/api/src/authz.rs` exists and is the only definition of each predicate.
- [ ] No `require_*` helper remains in any `*/routes.rs`.
- [ ] `grep -rn "get_member\|get_channel_member" backend/api/src/*/routes.rs` returns
      nothing — routes go through `authz`.
- [ ] Behaviour is byte-identical: the existing 233 HTTP tests pass unchanged.

## Tests

Move the authorization-matrix assertions into a shared helper in
`http_tests/common.rs` so every new endpoint can assert member-ok / non-member-forbidden
/ no-token / not-found in one line. Add direct unit tests for `ChannelAccess` covering
the guest, private and `group_dm` branches.
