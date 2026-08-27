# CS-044 — Session revocation must not fail open

**Wave:** 10 — Guest containment and operational readiness
**Area:** backend/api
**Blocked by:** —
**Blocks:** —
**Audit finding:** HIGH — security, undocumented failure mode

## Problem

[`middleware.rs`](../../backend/api/src/middleware.rs#L191) treats a Redis error during the
revocation lookup as "not revoked" and lets the request through. With
`ACCESS_TOKEN_EXPIRY` defaulting to 3600, a revoked session — somebody suspended, somebody
deprovisioned through SCIM, a token pulled after a compromise — keeps working for up to an
hour whenever Redis is unavailable or slow enough to error.

CS-033 sells deprovisioning as "removing them in the identity provider ends their access
here". That promise quietly does not hold during a Redis incident, and nothing tells the
operator it is not holding: the lookup failure is a `warn!` line with no metric and no
alert.

The refresh tokens are in Postgres and are deleted on revoke, so the blast radius is bounded
by the access-token lifetime. That bound is the part worth shortening.

## Approach

1. **Shorten the default access token to 15 minutes.** The refresh flow already exists and
   is exercised on every reload, so this costs a round trip and caps the exposure at a
   quarter of an hour instead of an hour. Keep it configurable; change the default, because
   defaults are what people run.
2. **Make the revocation check fail closed for the paths that matter, not for everything.**
   A blanket fail-closed turns a Redis blip into a total outage. The honest split: a lookup
   error rejects the request with `503` and a `Retry-After`, because a client that retries
   in a second is a better outcome than a fired employee reading messages for an hour — but
   only after the store has had a chance to answer. Implement it as a short timeout plus one
   retry, so a slow Redis is not the same as a missing one.
3. **Count it and alert on it.** `auth_revocation_lookup_failures_total`, plus a line in the
   RUNBOOK alert table: any sustained non-zero rate means revocation is not being enforced.
4. **Write down the trade.** Whatever the final choice, the failure mode belongs in
   RUNBOOK next to the SCIM section, where somebody deprovisioning a person will read it.

## Acceptance

- [ ] `ACCESS_TOKEN_EXPIRY` defaults to 900.
- [ ] A revocation lookup that cannot reach Redis refuses the request rather than allowing it.
- [ ] A slow-but-alive Redis does not refuse requests.
- [ ] `auth_revocation_lookup_failures_total` is exposed and documented as an alert.
- [ ] The failure mode is described in RUNBOOK beside SCIM deprovisioning.

## Tests

`http_tests/auth.rs`: point the middleware at a Redis that errors and assert a revoked
token is refused rather than accepted, and that the response is a `503` and not a `401` —
the difference matters to a client deciding whether to log the person out. Assert the metric
increments. A test that a healthy Redis path is unchanged.
