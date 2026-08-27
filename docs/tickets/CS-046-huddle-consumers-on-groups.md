# CS-046 — Move the huddle and call consumers onto consumer groups

**Wave:** 10 — Guest containment and operational readiness
**Area:** backend/api (worker) · backend/realtime
**Blocked by:** ~~CS-028~~ ✅ shipped (Redis Streams + consumer groups)
**Blocks:** —
**Audit finding:** MEDIUM — single point of failure

## Problem

CS-028 moved the notification and hook consumers onto `XREADGROUP`, which is what makes
multiple `chat-worker` replicas safe. Two consumers did not move and are still on plain
pub/sub, which delivers to every subscriber:

- [`huddle/consumer.rs`](../../backend/api/src/huddle/consumer.rs) — records joins and
  leaves and ends the session when the last participant leaves.
- `notifications::consumer::start_call_consumer` — rings the person being called.

Run two replicas and every join is recorded twice, `record_leave` races with itself over
who ends the session, and an incoming huddle rings twice. So the worker stays at one
replica, and that one process is a single point of failure for huddle history and for call
ringing: if it dies, calls stop ringing while everything else keeps working — the worst
shape of outage, because nothing looks broken.

At thirty-five people this is not a capacity problem. It is a resilience problem, and it is
the last thing standing between the worker and horizontal scale.

## Approach

1. **The events already exist on the stream; use them.** CS-028 kept ephemeral events on
   pub/sub deliberately — a replayed typing indicator is not recovery. Huddle membership is
   not ephemeral: it is history, it is written to a table, and it belongs in the log with
   the other durable events. The ring is the ephemeral half and can stay on pub/sub *if*
   the duplicate is made harmless instead.
2. **Two different fixes for two different events**, rather than one blunt one:
   - `huddle.member_joined` / `huddle.member_left` → durable, read through `StreamGroup`
     with acknowledgement, exactly like notifications.
   - `huddle.ring` → stays live, but the notification row it creates is claimed by
     `(huddle_id, to_user_id)` before it is written, the way CS-028 claims
     `(hook_id, event_id)`. A second replica loses the race and does nothing.
3. **`end_session` must be idempotent** regardless of transport. `record_leave` returning
   zero remaining participants is a race between replicas; make ending the session a
   conditional update that only the first caller wins, and stop inferring it from a count.
4. **Then say so.** The claim in RUNBOOK that the worker can scale was wrong until this
   ships; when it does, replace the caveat with the number of replicas that are actually
   safe and what still is not.

## Acceptance

- [ ] Huddle joins and leaves are read through a consumer group with acknowledgement.
- [ ] Two worker replicas record one join per join and ring once per call.
- [ ] Ending a session is idempotent under concurrent leaves.
- [ ] RUNBOOK states the supported replica count instead of a caveat.

## Tests

`http_tests/huddle.rs`: two `StreamGroup` consumers in the same group see each join once,
the way `two_worker_replicas_each_see_an_event_once` already proves for notifications.
A test that two concurrent `record_leave` calls for the last two participants end the
session exactly once.
