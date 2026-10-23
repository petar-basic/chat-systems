# CS-037 — SFU for large huddles

**Wave:** 20 — dialogs and large calls
**Area:** backend · frontend · ops
**Blocked by:** —
**Blocks:** —

## Problem

Huddles use a WebRTC mesh: every participant sends their stream to every other
participant. Upstream bandwidth and CPU grow with `n − 1` per person, so the practical
ceiling is six to eight participants. An all-hands for 70 people does not exist as a
feature.

The mesh is the right choice for the common case — a two- or three-person call needs no
server-side media and no extra infrastructure — so this is an addition, not a replacement.

## Approach

Four parts, each its own PR, in this order.

### 1. LiveKit beside the stack, and tokens from the API

- **LiveKit as the SFU**, self-hosted as its own compose file, `docker-compose.livekit.yml`,
  deployed beside the app the way `docker-compose.turn.yml` deploys coturn (a separate Coolify
  resource on the same host). It is Apache-2.0 and has a Rust server SDK. It uses the
  existing coturn for clients behind strict NAT, so it opens no TURN of its own: TCP 7880
  (signalling, behind Caddy as `wss://`), TCP 7881 and a bounded UDP range for media. The
  ports and the range are documented next to coturn's.
- **Configuration**: `LIVEKIT_URL`, `LIVEKIT_API_KEY`, `LIVEKIT_API_SECRET` and
  `HUDDLE_SFU_THRESHOLD` (default 6). Without the URL the SFU is off and every call stays on the
  mesh, as today.
- **The API mints LiveKit access tokens**: `POST /huddles/{id}/sfu-token` signs a token for
  the room and the caller, after the same check the realtime gateway runs on `huddle.join`
  ([`ws_handler.rs`](../../backend/realtime/src/ws_handler.rs), `"huddle.join"`: channel
  membership, or both people in the workspace for a DM) — moved into `authz` so the two
  cannot drift. The SFU never makes an access decision; it validates a token this API signed.
  An SFU with its own permission model is a second authorization system to keep in sync.
- **The room** is named after the huddle and recorded in `huddles.livekit_room`, the column
  the initial schema already carries for it and nothing has used.

### 2. One huddle, two transports

- `MeshManager` ([`lib/MeshManager.ts`](../../frontend/src/features/huddle/lib/MeshManager.ts))
  goes behind a `HuddleTransport` interface — join, leave, publish camera / microphone / screen,
  mute, the remote tracks, the active speaker — and `SfuTransport` implements it with
  `livekit-client`. `HuddleController` and `HuddleWindow` only see the interface.
- Every huddle starts on the mesh; part 3 moves it when it grows. In this part the SFU path is
  reachable behind a development flag, so the transport can be built and tested on its own.
- Mute, camera, screen share, raised hand and reactions keep going over the realtime gateway
  as now, so the huddle history and the system messages do not care which transport carried
  the media. Speaker view and active-speaker detection come from LiveKit on the SFU and from
  `useSpeaking` on the mesh.

### 3. Promotion mid-call

- The gateway already counts members (`huddle_redis_join`). When a join takes a mesh huddle
  past the threshold, it sends `huddle.transport { transport: "sfu" }` to every member.
- Each client fetches a token, joins the room and publishes, and tears down its mesh peer
  connections only once the SFU tracks of everybody it was connected to are flowing — nobody
  hears silence during the switch. A client that cannot reach the SFU stays on the mesh with
  the people it still has and says so.
- A huddle never moves back to the mesh when people leave; the room closes when it ends.

This is the hard part of the ticket; budget for it and test it explicitly.

### 4. The ceiling, written down

- Measure what one LiveKit node supports on the deployed hardware (participants with
  microphones, with cameras, with a screen share) and put the numbers in the RUNBOOK, with the
  CPU and bandwidth they cost, before anybody schedules an all-hands.
- `docs/manual-qa.md` gets a scripted 10-participant call: start on the mesh, cross the
  threshold, share a screen, mute, leave and rejoin.

Server-side recording stays out of scope: it is a compliance feature with its own consent and
retention requirements, and folding it in would make this ticket unshippable.

## Acceptance

- [ ] Calls up to the threshold use the mesh with no SFU running, and an instance without
      `LIVEKIT_URL` behaves exactly as today.
- [ ] Calls above it use the SFU; promotion happens without dropping anybody's audio.
- [ ] LiveKit tokens are minted by the API after the same authorization the gateway uses.
- [ ] LiveKit is an optional compose file with its ports, resources and Coolify steps
      documented.
- [ ] Screen share, mute, raised hand, reactions and the huddle history work on both
      transports.
- [ ] The measured participant ceiling is in the RUNBOOK.

## Tests

Existing realtime huddle tests pass unchanged for mesh calls. `http_tests` for the token
route: a member gets a token scoped to the room and themselves, a non-member and a guest
outside the channel get 403, and the route is 404 while the SFU is off. Realtime tests for the
`huddle.transport` frame at the threshold and not below it. Frontend tests for the transport
switch with both transports faked. Promotion and media quality are manual, in
`docs/manual-qa.md`.
