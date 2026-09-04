# Roadmap & known limitations

What is not done, why, and in what order it should be built. The shipped work — twelve
waves of it — is written up in [history.md](./history.md); this page keeps what is open,
the reasoning behind the order, and the things deliberately left out. Every open item is a
ticket in [`docs/tickets/`](./tickets/INDEX.md); a ticket's file is deleted when it ships.

## Open

| Ticket | What | Status |
|---|---|---|
| [CS-047](./tickets/CS-047-prove-it-at-import-scale.md) | Prove it at import scale: a 500k-message corpus, ranked-search pagination, the importer, the search backfill and a restore actually executed | **Next.** Cheap, and it finds the unknowns where a migration would otherwise find them |
| [CS-037](./tickets/CS-037-huddle-sfu.md) | SFU (LiveKit) for huddles above the mesh's six-to-eight ceiling | On request. Adds a media server to the stack; nothing below a 20-person team needs it |

Two decisions were considered on 2026-09-02 and deferred with their reasoning below:
versioning the API path and asymmetric JWT signing.

## How the order was chosen

1. **Wave 0 before anything else.** It installs the regression net (E2E in CI) and the
   three structures the rest depends on: one authorization module, one session-revocation
   path, and background workers in their own process. Half the fixes below are one-line
   changes *given* those, and duplicated plumbing without them.
2. **Access control before rate limits.** A limit on a leaking endpoint is a slower leak.
3. **Governance after access control.** An audit log is only worth having once the thing
   it audits is enforced.
4. **Correctness before performance.** And within performance, the renderer before
   virtualization — windowing a list of editor instances optimizes the wrong layer.
5. **Durable delivery after the worker split.** Both rework the same transport.
6. **Compliance before parity features.** Retention, export and SSO are what a security
   review asks for; custom emoji is not.

---


## Shipped

One line per wave; the ticket-by-ticket account is in [history.md](./history.md) and the
"landed as" pointers in the [ticket index](./tickets/INDEX.md). There is no Wave 10: what
was pencilled in for it was absorbed into 9 and 11.

| Wave | Tickets |
|---|---|
| [Wave 0 — Safety net and structural groundwork](./history.md#wave-0--safety-net-and-structural-groundwork) | CS-001 · CS-002 · CS-003 · CS-004 · CS-005 |
| [Wave 1 — Access control](./history.md#wave-1--access-control) | CS-006 · CS-007 · CS-008 · CS-009 · CS-010 |
| [Wave 2 — Abuse and resource limits](./history.md#wave-2--abuse-and-resource-limits) | CS-011 · CS-012 · CS-013 · CS-014 · CS-015 |
| [Wave 3 — Authentication hardening](./history.md#wave-3--authentication-hardening) | CS-016 · CS-017 |
| [Wave 4 — Governance](./history.md#wave-4--governance) | CS-018 · CS-019 · CS-020 |
| [Wave 5 — Correctness](./history.md#wave-5--correctness) | CS-021 · CS-022 · CS-023 |
| [Wave 6 — Performance](./history.md#wave-6--performance) | CS-024 · CS-025 · CS-026 · CS-027 |
| [Wave 7 — Reliability](./history.md#wave-7--reliability) | CS-028 |
| [Wave 8 — Compliance](./history.md#wave-8--compliance) | CS-029 · CS-030 · CS-031 · CS-032 · CS-033 |
| [Wave 9 — Product parity](./history.md#wave-9--product-parity) | CS-034 · CS-035 · CS-039 · CS-040 · CS-036 |
| [Wave 11 — Guest containment, operational readiness and mobile](./history.md#wave-11--guest-containment-operational-readiness-and-mobile) | CS-041 · CS-042 · CS-043 · CS-044 · CS-045 · CS-046 · CS-038 |
| [Wave 12 — Architecture follow-ups](./history.md#wave-12--architecture-follow-ups--shipped) | durable outbox · generated API and frame contract · sqlx macros · object_store / garde / askama · one message model · god-file splits |

---

## What is deliberately not on this list

- **A password policy beyond length.** `CS-017` originally proposed a 12-character
  minimum, a breach-list check and rejecting passwords containing the user's own name.
  Decided against on 2026-08-08: the project does not want to police what people choose,
  and the minimum stays at 8. An organisation that needs a policy has SSO (`CS-032`), and
  the identity provider is where such a policy belongs.

- **Partitioning `messages` / `audit_log`.** Right at a scale this instance is nowhere
  near. Revisit when a metric says so, not before.
- **Server-side huddle recording.** A compliance feature with its own consent and retention
  requirements; it does not belong inside CS-037.
- **Cyrillic ↔ Latin transliteration in search.** A separate feature from CS-034's
  diacritic folding, and it should not be smuggled in with it.
- **SCIM `/Groups`.** `/Users` delivers the whole offboarding value; groups are where SCIM
  implementations go to die. Add only on request.
- **Versioning the API path (`/api/v1`).** Considered 2026-09-02 and deferred. It would
  break every incoming-webhook URL already pasted into somebody's CI, and touch nginx,
  Caddy, the E2E helpers and every route table in `docs/backend.md`, for a benefit that
  only materialises when a second, incompatible version exists. The OpenAPI document is
  the contract in the meantime; revisit when a breaking API change is actually planned,
  and do the version bump as part of it.
- **Asymmetric JWT signing (EdDSA) instead of a shared HS256 secret.** Considered
  2026-09-02 and deferred. It is the better design — the realtime gateway would hold only
  the public key, and a leaked gateway config could no longer mint tokens — but it makes
  every deployment generate and distribute a keypair, and changes `ci-env.sh`, compose and
  the runbook. Revisit if the gateway ever runs on a less trusted host than the API, or if
  a security review asks for it.
