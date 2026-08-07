# CS-016 — Uniform login failure and constant-time path

**Wave:** 3 — Authentication hardening
**Area:** backend/api
**Blocked by:** —
**Blocks:** —
**Audit finding:** S6 (MEDIUM)

## Problem

[`login`](../../backend/api/src/auth/service.rs#L73) leaks account existence twice.

**By message.** A pending account returns a distinct string:

```rust
if user.status != UserStatus::Active {
    return Err(AppError::Unauthorized(
        "Account is not active. Please complete registration first.".into()));
}
```

Anyone can enumerate which addresses have been invited to the instance by reading the
error text.

**By timing.** When the address is unknown, the function returns at `find_by_email`
without running Argon2. When it exists, it pays a full verification. Argon2 defaults are
tens of milliseconds — comfortably measurable over a network, so the address list is
recoverable even after the message is fixed.

`forgot_password` ([`auth/service.rs:224`](../../backend/api/src/auth/service.rs#L224))
already handles this correctly and returns `Ok(())` unconditionally. Login should match
it.

## Approach

1. **One failure message for every credential failure.** Unknown address, wrong password,
   pending account, suspended account — all return
   `AppError::Unauthorized("Invalid email or password")`. Keep a distinguishing
   `tracing::info` server-side with the actual reason so support can still diagnose.
2. **Always run the hash.** When no user is found, verify against a fixed dummy Argon2
   hash of a constant string, computed once at startup and stored on `AuthService`:
   ```rust
   fn verify_or_dummy(&self, password: &str, hash: Option<&str>) -> bool {
       let target = hash.unwrap_or(&self.dummy_hash);
       Self::verify_password(password, target).unwrap_or(false)
   }
   ```
   Same for an account with `password_hash = NULL` (invited but not registered), which is
   currently a fast path too.
3. **Rate-limit before the lookup, not after.** The per-email and per-IP limiters already
   run first ([`auth/routes.rs:52`](../../backend/api/src/auth/routes.rs#L52)) — keep that
   order, and note that with CS-013 the per-email key itself is a weak oracle only if
   429s differ by account. They do not, since the key is hashed input, not a lookup.
4. **Pending accounts still need a path forward.** Removing the helpful message removes a
   real signal for a user who was invited but never registered. Replace it with a flow
   that does not leak: the login page offers "resend invite" which posts to an endpoint
   that always returns 200, exactly like `forgot-password`.
5. **Same treatment for `/auth/invites/:token/verify`** — an invalid and an expired token
   should be indistinguishable.

## Acceptance

- [ ] Unknown, pending, suspended and wrong-password all return the same status and body.
- [ ] Median response time for unknown and known addresses is within noise.
- [ ] Server logs still record the real reason.
- [ ] A "resend invite" path exists that does not disclose account state.

## Tests

`http_tests/auth.rs`: assert byte-identical responses across the four failure cases. Add a
coarse timing assertion — 50 attempts each for a known and an unknown address, assert the
medians are within a wide tolerance. Keep the tolerance loose; the test guards against
the missing-hash regression, not against a lab-grade side channel.
