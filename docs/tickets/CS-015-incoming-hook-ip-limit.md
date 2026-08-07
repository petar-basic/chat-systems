# CS-015 — Per-IP limit for incoming webhooks

**Wave:** 2 — Abuse and resource limits
**Area:** backend/api
**Blocked by:** CS-012
**Blocks:** —
**Audit finding:** S13 (LOW-MEDIUM)

## Problem

[`incoming_webhook`](../../backend/api/src/hooks/routes.rs#L286) is the only
unauthenticated mutating endpoint in the API. Its rate limit is keyed on the token from
the URL:

```rust
&format!("rate_limit:hook_incoming:{token}")
```

A caller who varies the token gets a fresh bucket on every request, so the limiter caps
legitimate use of a valid hook and does nothing about traffic from an unknown source.
Each request still runs `find_active_incoming_hook_by_token` against Postgres before
being rejected.

Token guessing itself is not the risk — the token is 24 random bytes from `OsRng`
([`hooks/routes.rs:39`](../../backend/api/src/hooks/routes.rs#L39)), which is not
brute-forceable. The risk is an unauthenticated request that reaches the database, at
whatever rate the network allows.

## Approach

Two keys with different jobs: one bounds the source, one bounds the hook.

1. **Per-IP limit first, before any database access.** Take the client address the same
   way `login` does ([`auth/routes.rs:56`](../../backend/api/src/auth/routes.rs#L56)) and
   enforce `rate_limit:hook_ip:{ip}` at 120/min with `LimiterFailure::Open` (CS-013).
   Rejecting here means an unknown caller never touches Postgres.
2. **Keep the per-token limit** as the second check — it is the one that protects a
   channel from a misbehaving but legitimate integration.
3. **Trust the proxy header correctly.** The IP comes from `X-Forwarded-For`, set by
   nginx ([`nginx.conf`](../../frontend/docker/nginx.conf)). Parse the **left-most**
   entry only when the immediate peer is a trusted proxy; otherwise a caller can forge
   the header and get a fresh bucket per request, which is the bug this ticket is
   fixing. Add `TRUSTED_PROXIES` (CIDR list, default the Docker bridge network) to
   `AppConfig` and resolve the client IP through a small shared helper so `login` and this
   endpoint agree.
4. **Constant-time failure.** Return the same 401 and the same shape whether the token is
   unknown or the hook is disabled, so response differences do not become an oracle.
5. **Do not log the token.** Confirm the token does not reach `tracing` output or
   `hook_executions` — the execution log currently records the payload
   ([`hooks/routes.rs:333`](../../backend/api/src/hooks/routes.rs#L333)), which is fine,
   but the token must not join it.

## Acceptance

- [ ] Requests from one IP are capped before the token lookup runs.
- [ ] A forged `X-Forwarded-For` from an untrusted peer does not reset the bucket.
- [ ] The per-token limit still applies.
- [ ] Unknown token and disabled hook return identical responses.
- [ ] `TRUSTED_PROXIES` is documented in `.env.example`.

## Tests

`http_tests/hooks.rs`: hammer the endpoint with rotating unknown tokens from one IP and
assert 429 before the DB is reached; assert a spoofed `X-Forwarded-For` does not help.
Assert the responses for unknown and disabled hooks are byte-identical.
