# CS-008 — Revoke access tokens on password reset and change

**Wave:** 1 — Access control
**Area:** backend/api
**Blocked by:** ~~CS-003~~ ✅ shipped as `sessions.rs`
**Blocks:** —
**Audit finding:** S4 (HIGH)

## Problem

[`reset_password`](../../backend/api/src/auth/service.rs#L268) and
[`change_password`](../../backend/api/src/auth/service.rs#L297) both end with
`delete_user_refresh_tokens`. Neither touches `RevocationStore`, and neither disconnects
live sockets.

Access tokens are stateless JWTs with a one-hour default lifetime. So the sequence a user
takes precisely because their account is compromised —

1. attacker holds a stolen access token and an open WebSocket,
2. user notices and resets their password,
3. refresh tokens are deleted, so the attacker cannot mint new tokens,

— leaves the attacker with **up to an hour of continued read and write access**, plus a
live socket streaming new messages for the same window. Password reset is the one moment
where revocation has to work, and it is the one place it is not called.

## Approach

Both functions call `sessions::revoke`. The only design question is which sessions
survive.

1. **`reset_password` → `SessionScope::All`.** The user is coming through a mailed link
   and is not authenticated in the browser they are using. There is no session worth
   preserving and the whole point is to evict whoever else is holding one.
2. **`change_password` → `SessionScope::AllExcept(current_jti)`.** The user is
   authenticated and mid-session; logging them out of the tab they just used to change
   their password is the behaviour that makes people avoid changing passwords. Everything
   else dies. `AuthUser` already carries the access token's `jti`, so the handler has
   the survivor id without a signature change.
3. **Keep the existing refresh-token deletion** — `sessions::revoke` performs it, so the
   direct `delete_user_refresh_tokens` calls in both functions are removed rather than
   duplicated.
4. **Audit both.** Emit `auth.password_reset` and `auth.password_changed` to the audit
   log. Full coverage is CS-018; these two rows are cheap to add here and are the ones an
   incident review reaches for first.

## Acceptance

- [ ] `reset_password` revokes every session for the user.
- [ ] `change_password` revokes every session except the initiating one.
- [ ] Live WebSocket connections for revoked sessions are closed with a reason.
- [ ] An access token issued before the change returns 401 afterwards.
- [ ] The session performing `change_password` continues to work without re-login.

## Tests

`http_tests/auth.rs`: mint two access tokens for one user, call `change_password` with
one, assert the caller's token still works and the other returns 401. Call
`reset_password` and assert both return 401. Assert the revocation flag's TTL matches the
remaining lifetime of the revoked token rather than the configured default.
