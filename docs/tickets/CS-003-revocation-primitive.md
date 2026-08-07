# CS-003 — Session revocation and live-disconnect primitive

**Wave:** 0 — Safety net and structural groundwork
**Area:** backend/api · realtime
**Blocked by:** —
**Blocks:** CS-007, CS-008, CS-033

## Problem

The building blocks for cutting off a session exist but are wired to exactly one caller.

- [`RevocationStore`](../../backend/api/src/middleware.rs#L106) is a Redis flag checked on
  every HTTP request and on WebSocket upgrade.
- [`disconnect_user`](../../backend/realtime/src/connection_manager.rs#L253) closes every
  live socket for a user.
- They are joined by exactly one event: `user.suspended`, published from
  [`admin/routes.rs:141`](../../backend/api/src/admin/routes.rs#L141) and handled at
  [`event_consumer.rs:329`](../../backend/realtime/src/event_consumer.rs#L329).

Instance-admin suspension therefore works correctly. Nothing else does: password reset
(CS-008), workspace removal and channel removal (CS-007) all need the same "this user's
current access must stop now" action, and each would otherwise reinvent it.

Two secondary gaps in the primitive itself:

- The revocation TTL is `access_token_expiry` ([`admin/routes.rs:137`](../../backend/api/src/admin/routes.rs#L137)).
  If that env var is later raised, previously written flags expire early. The TTL should
  be derived from the token being revoked, not from current config.
- Revocation is all-or-nothing per user. Revoking on password change logs the user out of
  every device including the one performing the change, which is why it is tempting to
  skip. A `jti` allowlist would let one session survive.

## Why now

CS-007 and CS-008 are the two highest-value security fixes in Wave 1 and both are
one-liners *if* this primitive exists, or duplicated plumbing if it does not.

## Approach

Promote the ad-hoc pieces into one named operation with a clear contract.

1. **New module `backend/api/src/sessions.rs`.** Single entry point:
   ```rust
   pub enum SessionScope {
       All,
       AllExcept(Uuid),
   }

   pub async fn revoke(state: &AppState, user_id: Uuid, scope: SessionScope,
       reason: &str) -> AppResult<()>
   ```
   It performs three steps in order and is the only place that knows they belong
   together:
   - delete refresh tokens for the user (all, or all but the surviving `jti`),
   - write the Redis revocation flag with a TTL of `access_token_expiry` **as read from
     the claims being revoked**, not from live config,
   - publish `session.revoked` with `{ user_id, except_jti }`.
2. **Realtime handles `session.revoked`** in `event_consumer.rs` next to the existing
   `user.suspended` arm, calling `disconnect_user`. Keep `user.suspended` as its own
   event — it means something different to the frontend (show "account suspended"), and
   collapsing them loses that.
3. **`AllExcept` needs a per-session claim.** Access tokens already carry `jti`
   ([`middleware.rs:26`](../../backend/api/src/middleware.rs#L26)) but only refresh tokens
   populate it. Populate `jti` on access tokens too in
   [`generate_tokens`](../../backend/api/src/auth/service.rs#L305), and have
   `auth_middleware` check the revocation flag's `except_jti` before rejecting. Store the
   flag as a small JSON value rather than `1` so the exception survives the round trip.
4. **The frontend needs a reason.** Send a WebSocket close frame carrying the reason
   before dropping the socket, so `ws.ts` can show "signed out — password changed"
   instead of a silent reconnect loop. The reconnect logic must not retry on a
   revocation close code.

## Acceptance

- [ ] `sessions::revoke` is the only code path that writes the revocation flag.
- [ ] `admin::suspend_user` is rewritten to call it and behaves identically.
- [ ] `SessionScope::AllExcept` keeps the initiating session alive, verified end to end.
- [ ] Revoked sockets receive a close frame with a reason and the client stops retrying.
- [ ] Revocation TTL derives from the revoked token's remaining lifetime.

## Tests

`http_tests`: revoke with `All` → a previously valid access token returns 401 on the next
request; revoke with `AllExcept(jti)` → that token still works while a second one does
not. Realtime tests: publish `session.revoked` and assert the connection is closed with
the expected code.
