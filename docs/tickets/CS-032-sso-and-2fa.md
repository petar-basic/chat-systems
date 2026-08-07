# CS-032 — SSO (OIDC) and 2FA

**Wave:** 8 — Compliance
**Area:** backend/api · frontend
**Blocked by:** ~~CS-003~~ ✅ shipped as `sessions.rs`, CS-016, CS-017
**Blocks:** CS-033
**Roadmap:** existing item, expanded

## Problem

Authentication is email and password, with no second factor — not even for the instance
admin, who can delete any workspace
([`admin/routes.rs:25`](../../backend/api/src/admin/routes.rs#L25)).

For a company of 70 this is the item that fails a security review outright:

- no Google Workspace / Entra / Okta login, so passwords are managed per-person;
- no central offboarding — removing someone from the identity provider does nothing here,
  which is what CS-033 has to solve on top of this;
- no MFA anywhere.

The schema anticipates it: `user_identities` with `provider` and `provider_id` exists in
[migration 1](../../backend/migrations/20240305000001_initial_schema.sql#L24) and is
unused.

## Approach

OIDC first — it is the one that changes the operational story. TOTP second, for the
accounts that still use a password.

### OIDC

1. **Authorization Code with PKCE**, via the `openidconnect` crate. Configuration per
   instance (issuer URL, client id, client secret, scopes), discovered through the
   provider's well-known document rather than hard-coded endpoints.
2. **New routes**, outside `auth_middleware`: `GET /api/auth/oidc/start` and
   `GET /api/auth/oidc/callback`. State and PKCE verifier live in Redis with a short TTL,
   keyed by an opaque handle in a `SameSite=Lax` cookie, matching the existing cookie
   approach in [`auth/routes.rs:260`](../../backend/api/src/auth/routes.rs#L260).
3. **Link by verified email, and only by verified email.** On callback, require
   `email_verified` from the provider. Match an existing user by email and write a
   `user_identities` row; otherwise create one — subject to step 4. Never link on an
   unverified address: that is account takeover by signup.
4. **Provisioning policy** as instance config: `disabled` (only pre-invited users may sign
   in), `invite_only` (default, matches today's model), or `domain_allowlist` (auto-create
   for listed email domains). Keep the invite flow working alongside it.
5. **Once a user has an identity, disable their password** unless they are an instance
   admin. A break-glass local admin account is deliberate; a shadow password on every SSO
   user is not.
6. **Tokens stay ours.** The OIDC exchange ends by minting the same access and refresh
   tokens as `login` does, so every downstream concern — revocation (CS-003), the
   WebSocket handshake, multi-instance — is untouched.

### TOTP

7. **`totp-rs`**, secret stored encrypted at rest, with single-use recovery codes hashed
   the same way passwords are.
8. **Mandatory for instance admins**, optional per user, enforceable per workspace by an
   owner. An admin account without a second factor should be impossible to create once
   this ships.
9. **Enrolment and recovery flows** in the settings panel, and a step-up prompt at login
   between password verification and token issue — the tokens are minted only after the
   second factor passes.
10. **Audit everything** (CS-018): enrolment, disablement, recovery-code use, and every
    failed second-factor attempt.

## Acceptance

- [ ] A user can sign in through OIDC and lands with normal session tokens.
- [ ] Linking requires a provider-verified email.
- [ ] Provisioning policy is configurable and defaults to invite-only.
- [ ] SSO users cannot sign in with a password.
- [ ] TOTP can be enrolled, is mandatory for instance admins, and issues recovery codes.
- [ ] Recovery codes are single-use and hashed.
- [ ] All authentication events are audited.

## Tests

`http_tests/auth.rs` against a mock OIDC provider: successful login creates the identity;
unverified email is rejected; disabled provisioning rejects an unknown user. TOTP: correct
code passes, replayed code fails, recovery code works once. Assert an instance admin cannot
complete registration without enrolling.
