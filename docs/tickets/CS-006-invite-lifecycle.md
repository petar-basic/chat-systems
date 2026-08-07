# CS-006 — Invite lifecycle: expiry, max uses, email binding

**Wave:** 1 — Access control
**Area:** backend/api
**Blocked by:** —
**Blocks:** —
**Audit finding:** S1 (HIGH)

## Problem

Three gaps compound into one: every invite ever created is a permanent, unlimited,
transferable key to the workspace.

1. [`WorkspaceRepo::create_invite`](../../backend/api/src/workspace/repo.rs#L229) inserts
   `workspace_id, created_by, email, role, token` and never sets `max_uses` or
   `expires_at`. Both columns are nullable with no default
   ([migration 1](../../backend/migrations/20240305000001_initial_schema.sql#L68)).
2. [`claim_invite_use_tx`](../../backend/api/src/workspace/repo.rs#L259) treats
   `max_uses IS NULL` as unlimited.
3. [`accept_invite`](../../backend/api/src/workspace/service.rs#L121) checks expiry only
   `if let Some(expires)`, and **never compares `invite.email` to the accepting user's
   email**.

So a link mailed to one person can be redeemed by any authenticated user on the
instance, any number of times, forever, at whatever role the invite carries — including
`admin`. A forwarded onboarding mail is a permanent back door.

## Approach

Make the safe configuration the default and the permissive one explicit.

1. **Migration** — set defaults at the schema level so a future insert that forgets them
   is still safe, and backfill existing rows:
   ```sql
   ALTER TABLE workspace_invites
     ALTER COLUMN expires_at SET DEFAULT NOW() + INTERVAL '7 days';

   UPDATE workspace_invites
      SET expires_at = COALESCE(expires_at, created_at + INTERVAL '7 days'),
          max_uses   = COALESCE(max_uses, 1)
    WHERE expires_at IS NULL OR max_uses IS NULL;
   ```
   Backfilling invalidates every currently outstanding link. That is the intent — say so
   in the migration's commit message and in `RUNBOOK.md`.
2. **`CreateInviteRequest` gains two optional fields**, `expires_in_hours` and
   `max_uses`, both clamped in the route:
   - email-bound invite (`email` present): defaults `max_uses = 1`, `expires_in_hours = 168`.
   - link invite (`email` absent): `max_uses` required and capped at 100,
     `expires_in_hours` capped at 168. A link invite with no cap is not offered.
   Validation lives in `workspace::routes::create_invite` next to the existing
   `require_role` call; the repo just persists what it is given.
3. **Bind the invite to the invited identity.** In
   [`accept_invite`](../../backend/api/src/workspace/service.rs#L121), after loading the
   invite and before claiming a use:
   ```rust
   if let Some(invited_email) = invite.email.as_deref() {
       let user = self.repo.find_user_email(user_id).await?;
       if !user.eq_ignore_ascii_case(invited_email) {
           return Err(AppError::Forbidden("This invite was issued to a different address".into()));
       }
   }
   ```
   Compare case-insensitively — `users.email` is stored as given and the invite address
   is typed by an admin.
4. **Expiry check stops being conditional.** Treat a `NULL` `expires_at` as expired once
   the migration guarantees the column is populated, so a row that escapes the default
   fails closed.
5. **Surface the state.** `list_invites` already returns the rows; the Integrations /
   Members UI should show `expires_at`, `use_count / max_uses`, and mark exhausted or
   expired invites so an admin can see what is outstanding. Revocation already exists.

## Acceptance

- [ ] New invites always carry both `expires_at` and `max_uses`.
- [ ] Existing invites are backfilled; the change is noted in `RUNBOOK.md`.
- [ ] An email-bound invite redeemed by a different account returns 403.
- [ ] An expired invite returns 400 regardless of `use_count`.
- [ ] A link invite cannot be created without `max_uses`.
- [ ] The members UI shows expiry and remaining uses.

## Tests

`http_tests/workspace.rs`: redeem an email-bound invite with the wrong account → 403;
with the right account → 200. Redeem twice with `max_uses = 1` → second is 400. Create an
invite, set `expires_at` into the past, redeem → 400. Create a link invite without
`max_uses` → 400. Keep the existing single-use test — it already covers the counter.
