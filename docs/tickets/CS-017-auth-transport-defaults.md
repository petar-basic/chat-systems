# CS-017 — Password policy and mail transport defaults

**Wave:** 3 — Authentication hardening
**Area:** backend/api
**Blocked by:** —
**Blocks:** —
**Audit findings:** S19, S18 (LOW)

## Problem

**Password policy is length only.**
[`validate_password`](../../backend/shared/common/src/validation.rs#L23) accepts anything
from 8 to 128 characters. `password` and `12345678` both pass. For a 70-person instance
with no SSO yet (CS-032), the password *is* the whole authentication story.

**SMTP defaults to no TLS.** `SMTP_USE_TLS` defaults to `false`
([`config.rs:80`](../../backend/api/src/config.rs#L80)) and the `false` branch of
[`build_mailer`](../../backend/api/src/auth/service.rs#L26) uses
`AsyncSmtpTransport::builder_dangerous`, which sends `SMTP_USER` and `SMTP_PASSWORD` in
cleartext. That is correct for MailHog on `localhost:1025` and quietly wrong the moment
someone points `SMTP_HOST` at a real relay without flipping the flag. `AppConfig` already
warns about plaintext `PUBLIC_URL`
([`config.rs:113`](../../backend/api/src/config.rs#L113)) — the same care is missing here.

## Approach

### Password policy

1. **Raise the floor to 12 characters** and keep the 128 ceiling. Length beats
   composition rules; do not add "one uppercase, one digit" requirements, which push
   people toward `Password1!`.
2. **Reject known-breached and trivially weak passwords.** Add a compiled-in deny list of
   the most common few thousand passwords (`include_str!` a newline-separated list,
   matched case-insensitively after trimming). This catches the realistic failure mode
   without a network call.
3. **Reject passwords containing the local part of the user's email or their display
   name**, case-insensitively. `validate_password` gains an optional context parameter;
   the call sites in `complete_registration`, `reset_password` and `change_password`
   already have the user loaded.
4. **Surface strength in the UI, do not enforce it.** The registration and change-password
   forms show a meter; the server enforces the rules above. Keep the server messages
   specific — this is not a credential oracle, the user already knows their own password.

### Mail transport

5. **Default `SMTP_USE_TLS` to `true`.** Explicit `false` stays supported for MailHog.
6. **Fail fast on the dangerous combination.** In `AppConfig::from_env`, alongside the
   existing `JWT_SECRET` check: if `smtp_use_tls` is false and `smtp_host` is not
   `localhost`, `127.0.0.1` or `mailhog`, **panic** with an explanatory message. Cleartext
   credentials to a remote host is a misconfiguration, not a preference, and startup
   validation is the one place the project allows failing fast.
7. **Support STARTTLS explicitly.** `relay()` implies implicit TLS on 465; many corporate
   relays want STARTTLS on 587. Add `SMTP_TLS_MODE` with `implicit | starttls | none`,
   defaulting to `starttls`, and map it onto `AsyncSmtpTransport::relay` vs
   `starttls_relay`. Keep `SMTP_USE_TLS` accepted as an alias for one release so existing
   `.env` files keep working.
8. **Update `.env.example` and `RUNBOOK.md`** with the production values and the reason.

## Acceptance

- [ ] Passwords under 12 characters, on the deny list, or containing the email local part
      are rejected.
- [ ] The frontend shows a strength indicator on both password forms.
- [ ] `SMTP_TLS_MODE` defaults to `starttls`; `none` against a remote host aborts startup.
- [ ] `.env.example` documents the production mail configuration.

## Tests

`validation.rs` unit tests for each rejection rule including the context-based ones.
`config.rs` tests, following the existing `from_env_panics_on_weak_jwt_secret` pattern, for
the cleartext-to-remote-host panic and for `SMTP_USE_TLS` back-compat.
