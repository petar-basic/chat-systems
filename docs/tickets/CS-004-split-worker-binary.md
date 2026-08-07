# CS-004 — Split background workers into `chat-worker`

**Wave:** 0 — Safety net and structural groundwork
**Area:** backend · infra
**Blocked by:** —
**Blocks:** CS-012, CS-028, CS-030 — and any horizontal scaling of the API

## Problem

Every `chat-api` process spawns four background workers at startup
([`main.rs:92-165`](../../backend/api/src/main.rs#L92-L165)): the hook consumer, the
reminder checker, the notification consumer and the huddle consumer. All four are driven
by Redis **pub/sub**, which delivers a copy to every subscriber.

With more than one API replica:

| Worker | Result with N replicas | Source |
|---|---|---|
| Notification consumer | N notification rows per mention — the table has no unique constraint and the insert is unconditional | [`notifications/repo.rs:26`](../../backend/api/src/notifications/repo.rs#L26) |
| Hook consumer | N deliveries of every outgoing webhook | [`hooks/executor.rs:96`](../../backend/api/src/hooks/executor.rs#L96) |
| Reminder checker | N reminders — plain `SELECT` then `mark_delivered`, no locking | [`hooks/repo.rs:191`](../../backend/api/src/hooks/repo.rs#L191) |
| Scheduled dispatcher | correct — `FOR UPDATE SKIP LOCKED` | [`scheduled/repo.rs:93`](../../backend/api/src/scheduled/repo.rs#L93) |

So the README's "stateless API, scales horizontally" holds for HTTP handling and fails in
practice: the second replica duplicates user-visible side effects. The deployment is
pinned to one API instance, which means no HA for the API tier and no zero-downtime
deploys.

The scheduled dispatcher shows the pattern is understood — it just was not applied
retroactively.

## Why now

Any later ticket that adds a consumer (CS-007 adds membership events, CS-028 reworks the
whole transport, CS-030 adds a purge job) would otherwise be written into `main.rs` and
then moved. Move the boundary once, before anything new is attached to it.

## Approach

A third binary in the existing Cargo workspace, alongside `chat-api` and `chat-realtime`.

1. **New crate `backend/worker/`** with the same shape as the other two: `main.rs`
   builds `AppState`, then spawns the consumers under the existing `supervise` helper.
   The consumer and executor modules themselves do not move — they stay with their
   feature under `backend/api/src/<feature>/`, which is where the conventions put them.
   To let the worker call them, promote the shared pieces (`AppState`, `config`, the
   feature modules) into a library target: give the `api` crate a `lib.rs` exposing them
   and keep `main.rs` as a thin binary over it. The worker then depends on `chat-api` as
   a library.
2. **`chat-api` stops spawning them.** The four `tokio::spawn` blocks in
   [`main.rs`](../../backend/api/src/main.rs#L92-L165) move to the worker's `main.rs`
   verbatim, including `supervise` and its backoff.
3. **The worker is single-replica by contract**, and that contract is written down: a
   `deploy.replicas: 1` in compose plus a startup log line naming it. This is the cheap
   correct answer for a 70-person instance. Do not build leader election now — if the
   worker later needs to scale, the fix is Redis Streams consumer groups, which CS-028
   introduces anyway.
4. **Defence in depth for the duplicate that hurts most.** Even with one worker, a
   restart mid-batch can re-deliver. Add a unique index that makes a duplicate
   notification impossible rather than merely unlikely:
   ```sql
   CREATE UNIQUE INDEX idx_notifications_dedup
     ON notifications (user_id, notification_type, (data->>'message_id'))
     WHERE data ? 'message_id';
   ```
   and switch the insert to `ON CONFLICT DO NOTHING`.
5. **Reminders get the same claim as scheduled messages.** Replace `get_due_reminders` +
   `mark_reminder_delivered` with a single `claim_due_reminders` using
   `UPDATE ... WHERE id IN (SELECT ... FOR UPDATE SKIP LOCKED) RETURNING *`, mirroring
   [`scheduled/repo.rs:93`](../../backend/api/src/scheduled/repo.rs#L93) so the two
   dispatchers read the same way.
6. **Compose and Dockerfile.** The backend Dockerfile already selects the binary via
   `ARG SERVICE`; add `chat-worker` to the build and a `worker` service to
   `docker-compose.yml`, `docker-compose.prod.yml` and `docker-compose.coolify.yml` with
   its own memory limit and a `/livez` probe.

## Acceptance

- [ ] `chat-api` spawns no background consumers.
- [ ] Running two `api` replicas produces exactly one notification, one webhook delivery
      and one reminder per event.
- [ ] `claim_due_reminders` is atomic and mirrors the scheduled-message claim.
- [ ] The notification dedup index exists and the insert is `ON CONFLICT DO NOTHING`.
- [ ] `worker` service is present in all three compose files with `replicas: 1`.
- [ ] `docs/backend.md` describes the three-process topology.

## Tests

`http_tests`: insert the same notification payload twice and assert one row. A worker
test that runs `claim_due_reminders` from two concurrent transactions and asserts each
reminder is claimed once. Manually verify the two-replica case once with
`docker compose up --scale api=2`.
