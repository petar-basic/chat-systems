# CS-012 — Write rate limit on every mutating router

**Wave:** 2 — Abuse and resource limits
**Area:** backend/api
**Blocked by:** CS-004 (touches `main.rs` router assembly)
**Blocks:** —
**Audit finding:** S11 (MEDIUM)

## Problem

`write_rate_limit` counts `POST`/`PUT`/`PATCH`/`DELETE` per user, 120 per minute
([`rate_limit.rs:14`](../../backend/api/src/rate_limit.rs#L14)). It is attached to
exactly two routers:

- [`messaging`](../../backend/api/src/messaging/routes.rs#L285)
- [`files`](../../backend/api/src/files/routes.rs#L47)

It is **not** attached to `conversations`, `workspace`, `hooks`, `scheduled`, `huddle`,
`notifications` or the protected half of `auth`. So sending direct messages — the most
spammable surface in the product — is unlimited, as is creating invites, creating
channels, creating workspaces and scheduling messages.

The per-router opt-in is the bug. A limit you have to remember to add is a limit that will
be missing on the next feature.

## Approach

Invert the default: rate limiting is applied once, centrally, and routers opt *out*.

1. **Apply it in `build_app`**, alongside the other cross-cutting layers in
   [`main.rs:229-241`](../../backend/api/src/main.rs#L229-L241):
   ```rust
   .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit::write_rate_limit))
   ```
   Order matters — it must sit inside `auth_middleware` so `AuthUser` is present.
   Since each feature router applies its own `auth_middleware`, the cleanest arrangement
   is to keep the write limiter on the `/api` nest and have it no-op when `AuthUser` is
   absent, which is what it already does today.
2. **Remove the two per-router copies** so there is one attachment point.
3. **Per-class budgets instead of one global number.** A single 120/min is simultaneously
   too loose for invites and too tight for a fast typist reacting to a thread. Introduce
   a small class table in `rate_limit.rs`:

   | Class | Limit | Applies to |
   |---|---|---|
   | `message` | 120 / min | channel and conversation message create, thread replies |
   | `reaction` | 240 / min | reactions add/remove |
   | `invite` | 20 / hour | invite create |
   | `workspace` | 5 / hour | workspace create |
   | `channel` | 30 / hour | channel create |
   | `default` | 120 / min | everything else mutating |

   Resolve the class from the matched route path, not from a per-handler call, so
   handlers stay free of limiter code. Keep the Redis key shape
   `rate_limit:write:{class}:{user_id}`.
4. **Return `Retry-After`.** `AppError::TooManyRequests` currently produces a bare 429;
   add the header so the client can back off instead of hammering. The frontend's
   `ApiClient` should honour it rather than retrying immediately.
5. **Make the limits configurable** through `AppConfig` with the table above as defaults,
   so an instance with unusual traffic can tune without a rebuild.

## Acceptance

- [ ] Every mutating route under `/api` is rate limited, verified by a test that
      enumerates the router and asserts coverage.
- [ ] No feature router attaches the limiter itself.
- [ ] Invite creation is capped per hour.
- [ ] 429 responses carry `Retry-After` and the client respects it.
- [ ] Limits are configurable via env with documented defaults.

## Tests

`http_tests`: drive each class past its limit and assert 429 plus the header. Add a test
that walks the assembled router and fails if a mutating route resolves to no class —
that is the test which keeps this from regressing when the next feature is added.
