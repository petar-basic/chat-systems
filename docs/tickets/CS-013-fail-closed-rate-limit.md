# CS-013 — Fail-closed rate limiting on auth paths

**Wave:** 2 — Abuse and resource limits
**Area:** backend/api
**Blocked by:** CS-012
**Blocks:** —
**Audit finding:** S12 (LOW-MEDIUM)

## Problem

[`rate_limit::enforce`](../../backend/api/src/rate_limit.rs#L36) swallows Redis errors and
allows the request:

```rust
Err(e) => {
    tracing::warn!("rate limit check failed (failing open): {}", e);
    return Ok(());
}
```

Failing open is the right call for message sending — a Redis blip should not stop the
company from talking. It is the wrong call for `/auth/login`, where the limiter is the
only thing between an attacker and unlimited password guesses. Right now a Redis outage
silently removes brute-force protection, and the only signal is a `warn` line.

## Approach

Make the failure mode a property of the call site rather than of the limiter.

1. **Add the policy to the signature:**
   ```rust
   pub enum LimiterFailure {
       Open,
       Closed,
   }

   pub async fn enforce(conn: &mut ConnectionManager, key: &str, max: u64,
       window_secs: u64, on_failure: LimiterFailure) -> AppResult<()>
   ```
   On a Redis error with `Closed`, return
   `AppError::ServiceUnavailable("Authentication is temporarily unavailable")` — a 503,
   not a 429. The client should not interpret it as "you were too fast".
   Add the `ServiceUnavailable` variant to `AppError` if it does not exist.
2. **`Closed` for the auth paths:** `/auth/login` (both the per-email and per-IP keys) and
   `/auth/forgot-password`. `Open` everywhere else, including the write limiter from
   CS-012 and the incoming-webhook limiter.
3. **Make the outage visible.** Increment a
   `rate_limit_backend_failures_total{policy}` counter next to the existing `warn`, and
   list it in `RUNBOOK.md` as an alert worth wiring — a limiter that has quietly stopped
   limiting is exactly the condition nobody notices.
4. **Keep the Lua script.** It is already atomic and correct; only the error branch
   changes.

## Acceptance

- [ ] Redis unavailable → login returns 503 and does not verify the password.
- [ ] Redis unavailable → sending a message still works.
- [ ] `rate_limit_backend_failures_total` is exported and documented in `RUNBOOK.md`.
- [ ] The 503 body does not reveal whether the account exists (see CS-016).

## Tests

`http_tests/auth.rs`: point the limiter at an unreachable Redis and assert login returns
503 while message send returns 200. Assert the counter increments.
