# Roadmap & known limitations

What is not done, why, and in what order it should be built. The shipped work — nineteen
waves of it — is written up in [history.md](./history.md); this page keeps what is open,
the reasoning behind the order, and the things deliberately left out. Every open item is a
ticket in [`docs/tickets/`](./tickets/INDEX.md); a ticket's file is deleted when it ships.

## Open

| Ticket | What | Status |
|---|---|---|
| [CS-077 – CS-088](./tickets/INDEX.md#wave-26--what-the-survey-found) | Wave 26: the 2026-10-27 product survey's findings — access holes, messages that open on the wrong screen, threads, failed sends, unread, notifications, reconnects, archive and export, invites, photos, Slack import | Wave 26, for 3.6.0 |
| [CS-037](./tickets/CS-037-huddle-sfu.md) | SFU (LiveKit) for huddles above the mesh's six-to-eight ceiling, promoted mid-call without dropping anybody | Held for the next major release (4.0.0), decided 2026-10-23 when 3.2.0 shipped CS-059 alone. Adds an optional media server beside the stack. Until then a huddle is capped at eight people |

Waves 21–23 were chosen in the 2026-10-23 review and shipped together in 3.3.0, before
4.0.0. Wave 24 came out of the 2026-10-26 review, which walked the app the way a company
trying it would, and shipped in 3.4.0. Wave 25, raised on 2026-10-27 after 3.4.2, shipped in
3.5.0. Two decisions were considered on 2026-09-02 and deferred
with their reasoning below: versioning the API path and asymmetric JWT signing.

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
| [Wave 12 — Architecture follow-ups](./history.md#wave-12--architecture-follow-ups--shipped) | durable outbox · generated API and frame contract · sqlx macros · object_store / garde / askama · one message model · god-file splits · CS-047 |
| [Wave 13 — What the scale measurement found](./history.md#wave-13--what-the-scale-measurement-found--shipped) | CS-048 |
| [Wave 14 — What a team from Slack misses on day one](./history.md#wave-14--what-a-team-from-slack-misses-on-day-one--shipped) | CS-049 · CS-050 · CS-051 · CS-052 |
| [Wave 15 — Links people actually paste at work](./history.md#wave-15--links-people-actually-paste-at-work--shipped) | CS-053 |
| [Wave 16 — The rest of what a team pastes](./history.md#wave-16--the-rest-of-what-a-team-pastes--shipped) | CS-054 |
| [Wave 17 — Files and images](./history.md#wave-17--files-and-images--shipped) | CS-055 |
| [Wave 18 — What a Slack user notices in the first hour](./history.md#wave-18--what-a-slack-user-notices-in-the-first-hour--shipped) | CS-056 · CS-057 |
| [Wave 19 — Getting around](./history.md#wave-19--getting-around--shipped) | CS-058 |
| [Wave 20 — Dialogs and large calls](./history.md#wave-20--dialogs-and-large-calls) | CS-059 (CS-037 open) |
| [Wave 21 — Notifications that fit how a team works](./history.md#wave-21--notifications-that-fit-how-a-team-works--shipped) | CS-060 · CS-061 · CS-062 |
| [Wave 22 — Finding files](./history.md#wave-22--finding-files--shipped) | CS-063 · CS-064 |
| [Wave 23 — Structured work: polls, lists, workflows](./history.md#wave-23--structured-work-polls-lists-workflows--shipped) | CS-065 · CS-066 · CS-067 |
| [Wave 24 — Ready for a company](./history.md#wave-24--ready-for-a-company) | CS-068, CS-069, CS-070, CS-071, CS-072, CS-073, CS-074, CS-075 |
| [Wave 25 — One person, several workspaces](./history.md#wave-25--one-person-several-workspaces) | CS-076 |

---

## What is deliberately not on this list

- **A password policy beyond length.** `CS-017` originally proposed a 12-character
  minimum, a breach-list check and rejecting passwords containing the user's own name.
  Decided against on 2026-08-08: the project does not want to police what people choose,
  and the minimum stays at 8. An organisation that needs a policy has SSO (`CS-032`), and
  the identity provider is where such a policy belongs.

- **Scale by headcount.** The 2026-10-26 review found that nothing has been measured above
  40 people: the member directory is sent whole to every client on start, and hundreds of
  concurrent connections, presence fan-out and push have not been load-tested. The RUNBOOK
  sizes an instance for a team of a few dozen, which is who uses it. Decided that day not to
  work on it until a team that size needs it; then measure first (a load test with a few
  hundred signed-in clients) and fix what the numbers show.
- **Partitioning `messages` / `audit_log`.** Right at a scale this instance is nowhere
  near. Revisit when a metric says so, not before.
- **Server-side huddle recording.** A compliance feature with its own consent and retention
  requirements; it does not belong inside CS-037.
- **Canvases.** Undecided as of 2026-10-23. Lists (CS-066) cover the structured half of
  what a canvas is used for; revisit once they ship and a team asks for free-form
  documents in the app.
- **OCR of images in file search.** CS-064 reads text that is in the file; reading text
  out of pictures needs a model beside the stack, and is a different feature.
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
