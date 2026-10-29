# CS-100 — Stored secrets get a key of their own

**Wave:** 28 — what 4.0.0 needs (2026-10-29 feedback on 3.7.0)
**Area:** backend · ops
**Blocked by:** —
**Blocks:** CS-101

## Problem

Two kinds of secret are encrypted at rest with a key hashed from `JWT_SECRET`
([`sealed.rs`](../../backend/api/src/sealed.rs), [`auth/totp.rs`](../../backend/api/src/auth/totp.rs)):
the OAuth client secrets and tokens of connected apps (Jira, Confluence, GitHub, Google Drive,
Figma) and every person's TOTP secret. Rotating `JWT_SECRET` — the first thing to do when it
leaks — therefore makes every connected app unreadable, so each has to be set up again and
every person has to reconnect, and it locks everyone with two-factor out of it. One secret
does two unrelated jobs, and the cheaper one to rotate holds the expensive one hostage.

## Approach

- **`SECRETS_KEYS`**: a list of `id:base64` 32-byte keys, the first one current. Required at
  start, with the same weak-value refusal `JWT_SECRET` has; `ci-env.sh`, compose, `.env.example`
  and the RUNBOOK generate and pass it.
- **Every sealed value records its key id** (`*_key_id` beside each `*_nonce`). Sealing uses the
  current key; opening uses the key the row names.
- **Rotation without downtime**: put a new key first, keep the old one listed, restart. The
  worker re-seals every row still on an old key, in batches, and logs when none is left; then
  the old key can be removed. A row whose key is gone fails loudly with its id rather than
  reading as empty.
- **Upgrading**: on its first 4.0.0 start the worker re-seals what 3.7.0 sealed under
  `JWT_SECRET`, so `JWT_SECRET` must still be set for that start; the RUNBOOK says when it can
  go (CS-101 removes it).
- `sealed.rs` becomes the one sealing module; `totp.rs` uses it with its own purpose.

## Acceptance

- [ ] Rotating the session signing key leaves connected apps and two-factor working.
- [ ] Rotating `SECRETS_KEYS` re-seals everything in the background and nobody reconnects.
- [ ] An instance upgraded from 3.7.0 keeps its connected apps and two-factor.

## Tests

Unit: seal and open across two keys, an unknown key id. API: a connected app and a TOTP
enrolment survive a key rotation and the worker's re-seal; the upgrade path from a value
sealed the 3.7.0 way.
