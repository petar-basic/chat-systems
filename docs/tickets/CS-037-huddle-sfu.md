# CS-037 — SFU for large huddles

**Wave:** 9 — Product parity
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —
**Roadmap:** existing item

## Problem

Huddles use a WebRTC mesh: every participant sends their stream to every other
participant. Upstream bandwidth and CPU grow with `n − 1` per person, so the practical
ceiling is six to eight participants. An all-hands for 70 people does not exist as a
feature.

The mesh is the right choice for the common case — a two- or three-person call needs no
server-side media and no extra infrastructure — so this is an addition, not a replacement.

## Approach

1. **Keep the mesh below a threshold** (default 6, configurable) and switch to an SFU above
   it. Most calls never leave the mesh path, which keeps the zero-infrastructure default
   intact for small deployments.
2. **LiveKit as the SFU**, self-hosted alongside the stack as an optional compose profile,
   the way `coturn` and MinIO already are. It is Apache-2.0, has a Rust server SDK, and the
   `huddles` table already carries an unused `livekit_room` column
   ([migration 1, line 192](../../backend/migrations/20240305000001_initial_schema.sql#L192))
   — the schema anticipated this.
3. **The API mints LiveKit access tokens**, scoped to the room and the participant, after
   running the existing huddle authorization
   ([`huddle/routes.rs:183`](../../backend/api/src/huddle/routes.rs#L183)). The SFU never
   makes an access decision; it validates a token this API signed. Keep it that way — an
   SFU with its own permission model is a second authorization system to keep in sync.
4. **Promotion mid-call must not drop anyone.** When the seventh participant joins, existing
   peers migrate from mesh to SFU. Implement as: create the room, have each participant
   publish to it, and tear down mesh peer connections only once the SFU tracks are flowing.
   This is the hard part of the ticket; budget accordingly and test it explicitly.
5. **Frontend.** `HuddleController` gains a transport abstraction so `HuddleWindow` does not
   care which is in use. Speaker view and active-speaker detection come from LiveKit above
   the threshold and from the existing local detection below it.
6. **Server-side recording is out of scope** — it is a compliance feature with its own
   consent and retention requirements, and folding it in here would make an already large
   ticket unshippable. Note it as a follow-up.
7. **Document the ceiling either way.** Whatever the SFU supports on the deployed hardware
   is a number operators need before they schedule an all-hands, not after.

## Acceptance

- [ ] Calls up to the threshold use the mesh with no SFU running.
- [ ] Calls above it use the SFU; promotion happens without dropping participants.
- [ ] LiveKit tokens are minted by the API after its own authorization check.
- [ ] The SFU is an optional compose profile with documented resource requirements.
- [ ] Screen share, mute, and the huddle history events work on both transports.
- [ ] Participant ceilings are documented.

## Tests

Existing realtime huddle tests must pass unchanged for mesh calls. Add tests for token
minting and for authorization refusal. Promotion and multi-party media quality are manual —
add a scripted scenario to `docs/manual-qa.md` covering a 10-participant call, screen share
and mid-call promotion.
