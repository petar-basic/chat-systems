# CS-101 — Sessions signed with EdDSA, with keys that rotate

**Wave:** 28 — what 4.0.0 needs (2026-10-29 feedback on 3.7.0)
**Area:** backend · realtime · ops
**Blocked by:** CS-100
**Blocks:** —

## Problem

Access and refresh tokens are HS256 with the shared `JWT_SECRET`
([`auth/service/tokens.rs`](../../backend/api/src/auth/service/tokens.rs)). The realtime gateway
holds the same secret to verify them, so a leaked gateway configuration can mint a session for
anybody, instance admins included. Rotating the secret signs everybody out at once, because
there is one key and no way to say which key signed a token. The ROADMAP deferred this on
2026-09-02 until a security review asked; one has.

## Approach

- **EdDSA (Ed25519)** for every token the API signs: access, refresh, registration and reset.
- **`JWT_SIGNING_KEYS`** (API and worker): Ed25519 private keys in PEM with a key id, the first
  one current. **`JWT_PUBLIC_KEYS`** (gateway): the public halves only. The token header
  carries `kid`; verification picks the key by it and refuses anything not EdDSA.
- **Rotation without signing anybody out**: add a new key first, keep the old one until the
  longest refresh token it signed has expired, then remove it. The RUNBOOK gives the commands
  (`openssl genpkey -algorithm ed25519`) and the timing.
- **`JWT_SECRET` goes**: nothing signs or verifies with it any more. Only the upgrade
  re-seal of CS-100 reads it, and the RUNBOOK says to remove it once the worker logs that no
  secret is left on it.
- Remove the EdDSA entry from the ROADMAP's *deliberately not on this list*.

## Acceptance

- [ ] The gateway verifies sessions without holding anything that can sign one.
- [ ] Adding a signing key and retiring the old one signs nobody out.
- [ ] An HS256 token, a token with an unknown `kid` and one signed by a retired key are refused.

## Tests

API: tokens carry `kid` and verify; rotation with two keys; HS256 and `alg: none` refused;
refresh across a rotation. Realtime: the gateway accepts an EdDSA token with the public key
alone and refuses the rest. Config: missing or malformed keys refuse to start.
