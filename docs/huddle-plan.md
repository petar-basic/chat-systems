# Huddle Feature — Implementation Plan

Slack-style live voice/video rooms ("huddles") for channels and DMs.

## Locked feature set

- **Media:** audio + video (camera toggle) + screen share. Topology = **mesh peer-to-peer**, max ~8–10 participants. No SFU / media server.
- **Audio controls:** mute toggle, device picker (mic/speaker/camera), speaking indicator (audio-level ring).
- **Video layout:** active-speaker focus, grid/gallery, pin participant, self-view tile.
- **In-huddle:** floating emoji reactions, hand raise, virtual backgrounds (client-side blur/replace).
- **Scope:** huddles in channels AND DMs/group-DMs; live participant list; invite mid-huddle.
- **Discovery:** channel banner ("Huddle active · N people · Join"), system message ("X started a huddle"), incoming-call ring for DM huddles (accept/decline), push notification to invited/mentioned users.
- **Cut:** screen annotation, huddle text thread, live captions/transcription, recording + AI notes, picture-in-picture mini window, background music, push-to-talk.

## Locked technical decisions

- **TURN/STUN:** self-host **coturn** (docker-compose). Backend mints short-lived time-limited TURN credentials.
- **Topology:** **multi-node ready** — directed signaling relayed via Redis pub/sub; Redis room set for cross-node membership.
- **History:** persist `huddle_sessions` + `huddle_participants` (missed-huddle log + analytics).
- **WebRTC:** hand-rolled perfect-negotiation, no new dependency.

Project rules: zero code comments; no backward-compat shims; follow existing feature-module + event conventions exactly.

---

## 1. Architecture overview

Each participant opens one `RTCPeerConnection` to every other participant (N·(N−1)/2 connections, capped ~8–10). All audio/video/screen flows peer-to-peer. The existing WebSocket service (`backend/realtime`) is the **signaling channel only** — it relays SDP offers/answers, ICE candidates, and control events; it carries zero media.

Two state planes:

- **Ephemeral** (realtime in-memory `DashMap` + Redis, mirroring the presence/typing pattern): live huddle membership and per-participant mute/camera/screenshare/hand flags. The Redis room set (`huddle:{id}:members`) makes membership answerable from any node.
- **Durable** (`backend/api` + Postgres): side effects that outlive the call — the "X started a huddle" system message, ring/push notifications, and the **history tables**.

```
 A renderer                 realtime (Axum WS)              B renderer
 ──────────                 ──────────────────              ──────────
  huddle.join ───────────▶  add to room (Redis + mem)
                            broadcast huddle.member_joined ─▶ render tile
  huddle.offer {to:B} ───▶  publish events:huddle ─────────▶ huddle.offer
                          ◀─ huddle.answer {to:A} ◀───────── B answers
  huddle.ice {to:B} ─────▶  publish events:huddle ─────────▶ huddle.ice
  ═══════════ direct P2P RTCPeerConnection media (audio/video/screen) ═══════════
  huddle.mute / .hand / .reaction ─▶ broadcast to room ────▶ update UI
```

Discovery (banner / system message / push) takes the normal API → Redis → notifications-consumer → realtime route, not the signaling route.

---

## 2. Shared events

`shared/events` structs are documentation only — the wire is dynamic `serde_json::Value`, routed by string `event_type`, Redis channel = first dot-segment. All huddle events route to `events:huddle` (`publisher.rs:22`, `connection_manager.rs:317`).

**Class A — realtime-only signaling/control** (client → `ws_handler` → relayed; never touch API/DB). Pure JSON, no struct (like `typing.indicator`):

| `type` | Direction | Realtime action |
|---|---|---|
| `huddle.join` | C→S | add to room, broadcast `huddle.member_joined`, reply `huddle.members` snapshot |
| `huddle.leave` | C→S | remove, broadcast `huddle.member_left` |
| `huddle.offer` | C→S→peer | publish to `events:huddle` with `to_user_id` |
| `huddle.answer` | C→S→peer | publish to `events:huddle` with `to_user_id` |
| `huddle.ice` | C→S→peer | publish to `events:huddle` with `to_user_id` |
| `huddle.mute` | C→S | broadcast `{user_id, audio_muted}` to room |
| `huddle.camera` | C→S | broadcast `{user_id, camera_on}` |
| `huddle.screenshare` | C→S | broadcast `{user_id, sharing}` |
| `huddle.reaction` | C→S | broadcast `{user_id, emoji}` (ephemeral) |
| `huddle.hand` | C→S | broadcast `{user_id, raised}` |

Server→client variants reuse the same `type` strings.

**Class B — API-originated lifecycle/discovery** (API → `events:huddle` → realtime `event_consumer`):

| `event_type` | Fan-out | Purpose |
|---|---|---|
| `huddle.started` | channel broadcast / both DM users | banner + system message |
| `huddle.ended` | same | clear banner |
| `huddle.ring` | `send_to_user(invitee)` | incoming-call modal |
| `huddle.invited` | `send_to_user(invitee)` | mid-huddle invite |

**Files:**
- `backend/shared/events/src/huddle_events.rs` — new; register `pub mod huddle_events;` in `backend/shared/events/src/lib.rs:1`. Doc-only structs `HuddleStarted`, `HuddleEnded`, `HuddleRing`, `HuddleInvited` (mirror `messaging_events.rs`).
- `frontend/src/lib/serverEvents.ts` — extend `AppServerEvent` union with the server→client variants.

---

## 3. Backend: realtime signaling

Core new work in `backend/realtime`.

**Connection state — `connection_manager.rs`:**
- `subscribed_huddles: HashSet<Uuid>` on `Connection` (line 18), init in `add_connection` (line 103).
- `huddle_members: DashMap<Uuid, HashSet<Uuid>>` (huddle_id → conn_ids) on `ConnectionManager` (line 25), parallel to `user_connections`.
- `join_huddle` / `leave_huddle` — mutate the `Connection` set and the index; idempotent.
- `broadcast_to_huddle` — `fan_out(msg, |c| c.subscribed_huddles.contains(&huddle_id))` (reuse `fan_out`, line 169). Local-only by design.
- `huddle_member_user_ids` — read room set for the snapshot.

**Cross-node membership:** Redis set `huddle:{huddle_id}:members` of user_ids, TTL refreshed on heartbeat (`presence_refresh`, line 256). Directed offer/answer/ice are published to `events:huddle` so the peer's node delivers via `send_to_user`. The Redis set answers "who is in this huddle" on any node.

**Dispatch — `ws_handler.rs` `handle_client_message` (line 173):** add arms per class-A `type`. Auth mirrors `typing.start` (lines 223–236): client sends scoping `channel_id` (or DM partner) with `huddle_id`; reuse `is_channel_member` / DM-pair check. Directed relay additionally verifies `to_user_id ∈ huddle_members[huddle_id]` (no SDP spray).

**Event consumer — `event_consumer.rs`:** add `"events:huddle"` to `channels` (line 25); arms in `handle_event` (line 75):
- Directed (`offer/answer/ice`, `ring`, `invited`): extract `to_user_id` → `send_to_user` (like `dm.created` line 218, `notification.push` line 166).
- Room broadcast (`member_joined/left`, `mute/camera/screenshare/reaction/hand`): extract `huddle_id` → `broadcast_to_huddle`.
- `started/ended`: extract `channel_id` → `broadcast_to_channel` (or both DM users).

**Cleanup (critical):** in `cleanup` (line 137) and `ConnGuard::drop` (line 37), extend presence teardown — iterate `subscribed_huddles`, remove conn from each `huddle_members` set, publish `huddle.member_left`; when a room empties, publish `huddle.ended`. Idempotent. Same shape as `presence_clear` → `publish_presence("offline")` (lines 140–143). Also remove the user from the Redis room set and (for history) emit the leave so the api consumer can stamp `left_at`.

---

## 4. Backend: api/huddle module

New `backend/api/src/huddle/`, declared in `main.rs:8` (`mod huddle;`), merged in `build_app` (line 188, alongside `dm::routes::router`). Mirrors `dm/`.

| File | Responsibility |
|---|---|
| `mod.rs` | `pub mod models; pub mod repo; pub mod routes; pub mod consumer;` |
| `models.rs` | `StartHuddleRequest { channel_id: Option<Uuid>, dm_partner_id: Option<Uuid> }`, `InviteRequest { user_ids: Vec<Uuid> }`, `RingRequest { user_id }`, `HuddleSummary`, `IceServersResponse`. `#[derive(Serialize, Deserialize)]`. |
| `routes.rs` | Handlers with `State<Arc<AppState>>` + `AuthUser`; `require_workspace_member` guard (copy `dm/routes.rs:41`) + `require_channel_access`. Endpoints below. |
| `repo.rs` | Writes `huddle_sessions` / `huddle_participants` (history is in scope). |
| `consumer.rs` | Subscribes `events:huddle`; persists lifecycle — `huddle.started` → insert session row; `member_joined/left` → upsert participant `joined_at` / stamp `left_at`; `huddle.ended` → stamp session `ended_at`. Mirrors `notifications/consumer.rs` subscribe loop. |

**Endpoints** (under `/api`, auth-guarded):
- `POST /workspaces/:ws_id/huddles` — start. Generate `huddle_id` (UUID, distinct from any `call_id`). Post system message via `state.message_repo.create(... metadata = {"kind":"huddle_started","huddle_id":...,"initiator_id":...})` (the `messages.metadata` JSONB column exists, `messaging/models.rs:11`) + `publisher.publish_message_created`. Then `publisher.publish("huddle.started", {...})`. DM huddles also fire `huddle.ring` to the partner. Returns `{huddle_id}`.
- `POST /workspaces/:ws_id/huddles/:huddle_id/invite` — validate invitees are workspace members; `huddle.invited` per invitee; ring-style invite also `huddle.ring`.
- `POST /workspaces/:ws_id/huddles/:huddle_id/ring` — (re)ring a user (DM accept/decline).
- `GET /workspaces/:ws_id/ice-servers` — return ICE servers: STUN + **time-limited coturn TURN credentials** (HMAC-SHA1 of `username = "<expiry-unix>:<user_id>"` with the shared coturn secret; see §10 coturn). Short TTL (default 12h). Client feeds these into `RTCPeerConnection({ iceServers })`. (Path is a `/workspaces/:ws_id` sibling, **not** under `/huddles/…`: matchit 0.7 — axum 0.7 — rejects a static `ice` segment as a sibling of the param `:huddle_id` used by the routes below.)

**Push/ring:** extend `notifications/consumer.rs` to also subscribe `events:huddle` (add to `channels`, line 27); on `huddle.ring`/`huddle.invited` create a `Notification` with the existing `NotificationType::Call` (`notifications/models.rs:13`) and publish `events:notification` — identical to the mention path (lines 70–151), respecting `is_dnd_active` and mute.

---

## 5. DB migrations

History is in scope → one migration following conventions (UUID PK `gen_random_uuid()`, `TIMESTAMPTZ NOT NULL DEFAULT NOW()`, append-only so no soft-delete):

```
backend/migrations/<ts>_huddle_history.sql
  huddle_sessions(
    id UUID PK, workspace_id UUID NOT NULL, channel_id UUID NULL,
    dm_partner_id UUID NULL, initiated_by UUID NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), ended_at TIMESTAMPTZ NULL)
  huddle_participants(
    huddle_id UUID NOT NULL REFERENCES huddle_sessions(id),
    user_id UUID NOT NULL,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), left_at TIMESTAMPTZ NULL,
    PRIMARY KEY (huddle_id, user_id))
```

Reuse `messages` + `metadata` for the system message and `notifications` + existing `NotificationType::Call` for ring/push — **no enum migration**. Do **not** touch any pre-existing `calls`/`call_participants` (SFU/livekit model, explicitly rejected).

---

## 6. Frontend: state + media

All under `frontend/src/`. Huddle state kept OUT of the workspace store (it must survive channel switches; `useRightPanel` resets on switch).

**`stores/huddle.ts`** (new):
```
activeHuddle: { huddleId, scope: {channelId} | {dmPartnerId}, workspaceId, initiatorId } | null
participants: Map<userId, { audioMuted, cameraOn, sharing, handRaised, speaking, pinned }>
incomingCalls: Array<{ huddleId, fromUserId, scope }>
localState: { audioMuted, cameraOn, sharing, selectedMic, selectedCamera, selectedSpeaker, background }
```
**`stores/workspace.ts`**: add `activeHuddleChannels: Map<channelId, {huddleId, initiatorId, startedAt}>` for the banner (discovery state belongs with channel state), updated on `huddle.started`/`huddle.ended`.

**`features/huddle/lib/MeshManager.ts`** (new, plain class): `Map<peerUserId, RTCPeerConnection>`; hand-rolled perfect-negotiation glare handling (deterministic "polite" peer by comparing user_id strings); signaling via the per-instance WS (`instanceManager.get(workspace.instanceUrl).ws`, never the global `wsClient`), subscribing `huddle.offer/answer/ice` on `globalEventBus`. One local `MediaStream` `addTrack`'d to every PC; screenshare as an extra track (`removeTrack` + renegotiate on stop); `replaceTrack` for camera/background toggles to avoid full-mesh renegotiation. ICE servers fetched from `GET /workspaces/:ws/huddles/ice`.

**Hooks** (`features/huddle/hooks/`):
- `useHuddleMedia.ts` — wraps `getUserMedia` / `getDisplayMedia`, Electron-aware via `lib/electron.ts` (`getDisplayMediaApproval()`).
- `useMediaDevices.ts` — `enumerateDevices`, react to `devicechange`, mic/speaker/camera lists; speaker via `audioEl.setSinkId`.
- `useSpeakingDetection.ts` — `AudioContext` + `AnalyserNode` per inbound stream, RMS threshold → `participants[userId].speaking`, throttled ~10 Hz.

**`features/huddle/lib/backgroundProcessor.ts`** (new): MediaPipe Selfie Segmentation → offscreen `<canvas>` → `canvas.captureStream()` → `replaceTrack`. Modes `none | blur | replace(image)`. Ship blur-first; degrade to `none` on frame-time spikes.

---

## 7. Frontend: UI components

Under `features/huddle/components/` unless noted.

- **HuddleBar** — "Huddle active · N · Join" / "Start huddle". Mounts in `features/channel/ChannelHeader.tsx` (action row) and `pages/DmView.tsx` header (~line 90). Reads `activeHuddleChannels`; buttons call `useStartHuddle` / `useJoinHuddle`.
- **HuddleWindow** — persistent overlay/dock in `pages/WorkspacePage.tsx`, gated on `activeHuddle != null`, z-index above `Modal` (z-50). NOT in `useRightPanel`. Contains:
  - `TileGrid` (gallery) + `ActiveSpeakerView` (focus) — switch driven by `speaking`; `pin` overrides.
  - `SelfViewTile` — local stream, mirrored.
  - `ParticipantList` — live from `participants`; mute/hand/speaking badges.
  - `HuddleControls` — mute, camera, screenshare, emoji-react (floating overlay), hand-raise, device-picker popover, leave. Each dispatches the matching `huddle.*` command.
- **IncomingCallRing** — built on `shared/components/Modal/Modal.tsx` (focus trap), `modal()` not `toast()` (toasts auto-dismiss). Mounts in `WorkspacePage.tsx` gated on `incomingCalls.length > 0`. Accept → join; Decline → `huddle.decline`.
- **InvitePicker** — reuse the @mention/member picker; calls `POST .../huddles/:id/invite`.

**`hooks/queries/useHuddle.ts`** (new): `useStartHuddle`, `useJoinHuddle`, `useInviteToHuddle`, `useAcceptHuddle`, `useDeclineHuddle`, `useIceServers`.

---

## 8. Discovery wiring

1. **Banner** — `lib/wsQuerySync.ts` adds `globalEventBus.on('huddle.started' | 'huddle.ended')` → patch `activeHuddleChannels`. Ensure listener cleanup on workspace switch.
2. **System message** — server-side in `huddle/routes.rs` start (`message_repo.create` with `metadata.kind="huddle_started"` + `publish_message_created`). `MessageItem` branches on `metadata.kind` to render "X started a huddle" with an inline Join button (net-new system-message rendering).
3. **Ring** — `huddle.ring`/`huddle.invited` → `send_to_user` → `features/notifications/NotificationStream.tsx` listener pushes to `incomingCalls` → `IncomingCallRing`; play ring sound via `lib/notifications.ts` `playNotificationSound`.
4. **Push** — `notifications/consumer.rs` on `events:huddle` `ring`/`invited` → `NotificationType::Call` notification + `events:notification` → existing `notification.push` path → `NotificationStream` → `showNotification` / Electron native (gated on `document.hasFocus() === false`). Respect `is_dnd_active` / mute.

---

## 9. Phased delivery

Each milestone independently shippable/testable. Realtime test harness: `backend/realtime/src/tests/common.rs` (mints tokens + fake conns; signaling relay is unit-testable without media).

- **M0 — Infra.** ✅ coturn in docker-compose (`turn` profile) + `GET /workspaces/:ws_id/ice-servers` creds endpoint + `huddle/` api module + `huddle_events.rs` + frontend event union. Redis room-set helpers deferred to M1 (added where `huddle.join`/`leave` first call them, to avoid landing dead code).
- **M1 — Audio 1:1 DM.** ✅ Realtime `join/leave/offer/answer/ice/mute` relay + Redis room-set membership + leave-on-disconnect cleanup. `MeshManager` (hand-rolled perfect-negotiation, N-peer-ready), `lib/media.ts` (mic), `HuddleController` (mounted in `App.tsx`), `HuddleWindow` (tiles + mute), `IncomingCallRing` (Modal), `HuddleStartButton` (DM header). Start endpoint (DM-only) + `huddle.ring`. Tests: 4 realtime + 4 api http green (full suites 54 + 228 pass); frontend tsc+eslint green. Deviations: media is a helper not a `useHuddleMedia` hook; start is DM-only until M2 adds the channel branch.
- **M2 — Channel huddle + full mesh audio (~8).** ✅ Channel start (`POST .../huddles` now takes `channel_id` XOR `dm_partner_id`, channel-access checked); N-peer mesh + `huddle.members` snapshot + participant list (M1 `MeshManager` already N-peer); speaking indicator (`useSpeaking` AudioContext analyser → green tile ring). Channel banner via `wsQuerySync` `huddle.started/ended` → workspace store `activeHuddleChannels` → `HuddleBar` in `ChannelHeader` (Start/Join). History: `huddle_sessions` + `huddle_participants` migration + `HuddleRepo` + `huddle/consumer.rs` (persists join/leave, emits `huddle.ended` when last participant leaves) spawned in `main.rs`; realtime relays `huddle.ended`. Tests: api huddle http now 8 (channel start, validation, history lifecycle) — 232 api + 54 realtime green; frontend tsc+eslint green. Deviations: in-channel system message ("X started a huddle") deferred to M5 (would touch the core `Message` struct + frontend interface); banner shows presence, not a live count N (a live count needs member changes broadcast to channel subscribers, not just huddle members — deferred).
- **M3 — Video + screen share.** ✅ Camera toggle (`huddle.camera`), screen share (`huddle.screenshare`, `getDisplayMedia` + Electron `setDisplayMediaRequestHandler` in `main.cjs`), self-view (mirrored), video grid + focus/pin layout + active-speaker (from store `speaking` Set), device picker (`useMediaDevices`: mic/camera/speaker; speaker via `setSinkId`). `MeshManager` gained `setVideoTrack` (addTrack-then-replaceTrack, single video sender) + `setAudioTrack` (mic swap). `HuddleController` owns mic/cam/screen tracks + exposes a `controls` object to `HuddleWindow`. Tests: realtime 55 (added `huddle.camera` relay), api 232; frontend tsc + eslint + vite build green. Deviations: **single video sender per peer** — a participant sends camera OR screen (screen takes precedence), not both simultaneously (keeps mesh simple; revert to camera on screen-stop). Electron screen share grants via `useSystemPicker` (OS picker), no custom source-picker UI.
- **M4 — Expression.** ✅ Floating reactions (`huddle.reaction`, quick-emoji picker, `huddle-float` CSS, transient store list w/ auto-expiry), hand raise (`huddle.hand`, ✋ tile badge), mid-huddle invite (`POST .../huddles/:huddle_id/invite` → `huddle.ring` per invitee; `InvitePicker` lists ws members not already in the huddle), **virtual backgrounds** (`@mediapipe/tasks-vision` selfie-segmentation → offscreen-canvas blur composite → `canvas.captureStream()` → `MeshManager.setVideoTrack`; `HuddleController` unifies camera/blur/screen track selection via `updateVideoOutput`; Sparkles toggle). Tests: api huddle http 10, realtime 55; frontend tsc+eslint+vite build green. ⚠️ Virtual backgrounds are **build-verified only** — segmentation/blur quality + perf are NOT runtime-tested (no camera/GPU here); MediaPipe WASM + `selfie_segmenter.tflite` load from CDN (jsdelivr + Google storage) at runtime — swap to self-hosted assets for an air-gapped deploy. Also: `PERSON_THRESHOLD` in `backgroundProcessor.ts` (mask foreground = `data[i] > 0`) is a best guess for the selfie model's category-mask convention; flip if blur lands on the person. Invite reuses `huddle.ring` (no separate `huddle.invited`); a channel-huddle invitee joins via dm-scope auth (both ws members) — explicit invite = access grant even for private channels.
- **M5 — Discovery polish.** ✅ Push notifications: notifications consumer subscribes `events:huddle`, on `huddle.ring` → persists a `NotificationType::Call` notification + pushes `events:notification` (→ native/Electron notif via existing `NotificationStream`), respecting `is_dnd_active`. Ring sound: `IncomingCallRing` plays `playNotificationSound` on mount + every 3s until dismissed. In-channel system message: channel-start posts a `messages.metadata.kind=huddle_started` row (`MessageRepo.create_system_message`, reuses the existing `metadata` JSONB column already on the `Message` struct) → `MessageItem` branches to `HuddleSystemMessage` ("X started a huddle" + Join button gated on the huddle still being active). Missed-huddle = the persisted `Call` notification (no separate UI). Tests: api huddle http 11 (added system-message assertion) → **235 api + 55 realtime**; frontend tsc+eslint+vite build green.

---

## 10. Risks / notes

- **Mesh scaling.** N·(N−1)/2 connections + per-peer encode; ~8–10 ceiling, heavy with video on. Consider auto-disabling outbound video above a threshold. Beyond this needs an SFU (cut).
- **coturn.** Self-hosted; add a `coturn` service to docker-compose with `use-auth-secret` + a `static-auth-secret` shared with the API. The API mints time-limited creds (`username = <unix-expiry>:<user_id>`, `credential = base64(hmac_sha1(secret, username))`) from `GET /huddles/ice`. Open UDP/TCP relay ports + `external-ip` in prod. This is the biggest ops dependency — stand it up in M0 before M1 leaves LAN.
- **Multi-node signaling.** Directed relay must publish to `events:huddle` (not local `send_to_user`) so peers on other nodes receive it; room membership lives in Redis (`huddle:{id}:members`).
- **Auth model.** `huddle_id ≠ channel_id`; client sends scoping `channel_id`/DM-pair with `huddle.join`, realtime checks membership; directed relay validates target is a current room member.
- **Electron screen-share.** macOS needs Screen Recording permission + `desktopCapturer`/`getDisplayMedia` via main process; renderer `getDisplayMedia` alone won't prompt. Wire main-process IPC in `lib/electron.ts` + Electron main. Web build uses standard `getDisplayMedia`.
- **Virtual background perf.** MediaPipe per-frame is GPU/CPU heavy; opt-in, offscreen canvas, degrade to `none` on spikes. Ship blur-first.
- **System-message rendering.** None exists today; `metadata.kind` branching in `MessageItem` is net-new (small).

### Key grounding files
realtime `connection_manager.rs`, `ws_handler.rs:173`, `event_consumer.rs:75`; api `main.rs:188`, `state.rs`, `dm/routes.rs`, `messaging/publisher.rs`, `messaging/models.rs:11`, `notifications/consumer.rs:27`, `notifications/models.rs:13`; shared `events/src/lib.rs:1`; frontend `serverEvents.ts`, `ws.ts`, `globalEventBus.ts`, `wsQuerySync.ts`, `stores/workspace.ts`, `ChannelHeader.tsx`, `DmView.tsx`, `WorkspacePage.tsx`, `NotificationStream.tsx`, `lib/electron.ts`.
