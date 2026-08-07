# CS-033 — SCIM deprovisioning

**Wave:** 8 — Compliance
**Area:** backend/api
**Blocked by:** CS-032 (identity linkage), CS-003 (revocation), CS-007 (membership removal)
**Blocks:** —

## Problem

With CS-032 shipped, people sign in through the identity provider. Nothing signs them out
of it. Removing someone from Okta or Entra stops new logins once their access token
expires, but leaves the account, its workspace memberships and its private-channel
memberships in place — and IT has no way to know the two systems disagree.

Offboarding a person then means a manual step in this product that somebody has to
remember on their last day. That is the step that gets missed.

This ticket is small precisely because CS-003 and CS-007 built the primitives. It is the
payoff for having done them first.

## Approach

SCIM 2.0, the subset that actually matters, over a bearer token.

1. **Endpoints** under `/api/scim/v2`, authenticated by a dedicated instance-level token
   (generated like hook tokens, revealed once, rotatable) and **not** by
   `auth_middleware` — the caller is a machine, not a user:
   - `GET /Users`, `GET /Users/:id` — filterable by `userName`
   - `POST /Users` — provision
   - `PATCH /Users/:id` — the important one: `active: false` is deprovisioning
   - `DELETE /Users/:id` — treat as deactivation, never as a hard delete; erasure is
     CS-031's job and must not be triggerable by an identity provider
2. **Deactivation is a composed operation**, reusing what exists:
   - set `users.status = 'suspended'`,
   - `sessions::revoke(All)` (CS-003) — kills tokens and live sockets,
   - remove workspace memberships through the CS-007 path so `channel_members` cascades
     and live subscriptions drop,
   - audit it (CS-018) with the SCIM caller as actor.
   Everything except the endpoint already exists. Do not reimplement any of it.
3. **Reactivation is not automatic.** `active: true` for a previously deactivated user
   restores the account but **not** their workspace or channel memberships — CS-007
   deliberately made removal destructive so that returning requires a fresh invite. Say so
   in the docs; an identity provider must not be able to silently restore access to private
   channels.
4. **Groups: out of scope, on purpose.** `/Groups` maps to workspace membership and is
   where SCIM implementations get complicated. Ship `/Users` first; it delivers the entire
   offboarding value. Add `/Groups` only if a customer asks.
5. **Rate limit and audit the endpoint** like any other unauthenticated-by-session surface,
   reusing the per-IP helper from CS-015.

## Acceptance

- [ ] `PATCH /Users/:id` with `active: false` suspends the account, revokes every session,
      closes live sockets and removes workspace and channel memberships.
- [ ] `DELETE` behaves as deactivation and never destroys data.
- [ ] Reactivation restores the account but grants no memberships.
- [ ] The SCIM token is separate from user sessions, revealed once and rotatable.
- [ ] Every SCIM mutation is audited with the caller identified.
- [ ] Responses conform to SCIM 2.0 well enough for Okta and Entra to validate.

## Tests

`http_tests`: deactivate via SCIM and assert the access token is rejected, the socket
closed, and no `workspace_members` or `channel_members` rows remain. Reactivate and assert
no memberships return. Assert an invalid or rotated token gets 401. Validate response
payloads against the SCIM schema.
