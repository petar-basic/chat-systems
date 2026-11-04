# CS-122 — `/api/v1`, and a client and a server that know each other's version

**Wave:** 33 — mobile apps (2026-11-04), released as 5.0.0
**Area:** backend · frontend · ops · docs
**Blocked by:** —
**Blocks:** CS-124 · CS-125

## Problem

Today the web client and the server are always the same version, because the server
serves the client. A phone app carries its own copy of the client (CS-125), so it will talk
to servers older and newer than itself, and nothing tells either side that the other has
moved on. The [roadmap](../ROADMAP.md) deferred versioning the API path until a breaking
change was planned; this is it. It is also why this release is 5.0.0.

## Approach

- Every API route moves under `/api/v1`, and the realtime socket under `/ws/v1`, in the
  route tables, the OpenAPI document, nginx, Caddy, the E2E helpers and `docs/backend.md`.
  The old paths are gone, not redirected. Incoming-webhook and bot URLs change with them:
  *Integrations* shows the new URL, and the CHANGELOG and `bot-api.md` say plainly that
  every pasted URL has to be updated.
- `GET /api/v1/instance/info` reports the API version and the oldest client version the
  server still serves. A client sends its version in `X-Client-Version`; a server that no
  longer serves it answers 426 with the version it needs, and the client says *This server
  needs a newer app*. A client that finds the server too old says that instead.
- The rule, written into `docs/backend.md`: within `v1`, fields and routes are added, never
  removed or changed in meaning; a change that breaks a client is `v2`, served beside `v1`
  for at least one major release.

## Acceptance

- [ ] Every route answers under `/api/v1` and nothing answers at the old path.
- [ ] An incoming webhook posts through its new URL, and *Integrations* shows that URL.
- [ ] A client older than the server's oldest is refused with 426 and a message that says
  what to do.
- [ ] The E2E suite runs green against the new paths.

## Tests

Backend: the version header and the 426, the old path gone, `instance/info`. Frontend: the
two messages. E2E: the whole suite on the new paths.
