# Manual QA / Visual Verification Guide

End-to-end manual test script for chat-systems. Covers the happy paths **and the
edge cases** that the automated suite can't easily click through (UI state,
realtime across tabs, file uploads, XSS, rate limits). Each case lists **how to
reach it**, the **steps**, the **expected result**, and **edge cases**.

Legend: 🟢 happy path · 🟠 edge case · 🔴 security/abuse case.

---

## 0. Bring the stack up

> Secrets are now required via env interpolation (`${VAR:?}`) — `docker compose`
> will refuse to start without them. This is intentional (no more committed
> secrets).

### Option A — full stack in Docker (closest to "deployed")
```bash
cp .env.example .env
# edit .env and set strong values:
#   JWT_SECRET=$(openssl rand -hex 32)
#   ADMIN_PASSWORD=<something strong>
#   POSTGRES_PASSWORD=<something strong>
docker compose --profile frontend up -d --build
# App:      http://localhost:3000
# MailHog:  http://localhost:8025   (catches all outgoing email)
```
- First admin: `admin@dev.local` / `<ADMIN_PASSWORD from .env>`.
- Health: `curl localhost:3000/livez` → `ok`; `curl localhost:3000/readyz` → `ready`.
- Metrics: `curl localhost:3000/metrics` → Prometheus text.

### Option B — dev mode (hot reload, two API ports)
```bash
# infra only
JWT_SECRET=dev-secret-key-min-32-chars-1234567890 ADMIN_PASSWORD=admin123456 POSTGRES_PASSWORD=devpassword \
  docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d postgres redis mailhog
# api + realtime on host (each in its own terminal), then:
cd frontend && npm install && npm run dev    # http://localhost:3001
```

### Seed test data
```bash
./seed.sh    # creates a test workspace + a few users (read the script for credentials)
```

**Pre-flight checklist before testing:** `/livez`, `/readyz` both green; you can log in as admin; MailHog reachable.

---

## 1. Authentication & onboarding

### 1.1 🟢 Login / logout
- **Reach:** open `/` (LoginPage).
- **Steps:** log in as admin → land on the workspace view → click avatar/menu → Logout.
- **Expected:** after login the workspace loads; after logout you're back at login and protected pages redirect to login.
- 🟠 **Edge:** refresh the page while logged in → you stay logged in (HttpOnly cookie / silent refresh). Wrong password → inline "Invalid email or password" (must NOT say whether the email exists).
- 🟠 **Edge — session refresh:** stay idle past the access-token lifetime (default 1h; or shorten `ACCESS_TOKEN_EXPIRY` to e.g. 60s and restart api) then perform any action → it should silently refresh and succeed, not log you out.
- 🔴 **Edge — concurrent tabs:** open the app in 3 tabs, let the token expire, then act in all 3 quickly → you should NOT get logged out (single-flight refresh). Before the fix this caused a spurious logout.

### 1.2 🟢 Invite → complete registration (the onboarding flow)
- **Reach:** as a workspace Owner/Admin, open the **Members** panel → "Invite" → enter an email.
- **Steps:** submit invite → open **MailHog** (`:8025`) → open the invite email → click the link (`/invite/<token>`) → the registration page shows the invited email + workspace → set display name + password → submit.
- **Expected:** account is created **and you're joined to the inviting workspace** (land inside it). The invite email link works (verify endpoint returns the invite info, not a 404).
- 🟠 **Edge — reused link:** click the same invite link again after registering → it should fail cleanly ("invalid/expired"), not create a duplicate or 500.
- 🟠 **Edge — tampered token:** change a character in the `/invite/<token>` URL → "invalid invite", no crash.
- 🟠 **Edge — password too short:** enter <8 chars → validation error, no submit.

### 1.3 🟢 Forgot / reset password
- **Reach:** LoginPage → "Forgot password" → enter email.
- **Steps:** submit → open MailHog → click reset link → set a new password → log in with it.
- **Expected:** new password works; **all existing sessions are revoked** (other tabs get logged out on next action).
- 🔴 **Edge — single-use:** click the SAME reset link a second time → must fail ("already used or expired"). (Reset tokens are now one-time.)
- 🔴 **Edge — enumeration:** request reset for an email that does NOT exist → the UI shows the same "email sent" success (no leak that the account doesn't exist); MailHog shows no mail.
- 🔴 **Edge — token confusion:** copy the reset-link token and try to use it as a normal session (e.g. paste as `Authorization: Bearer <token>` to `/api/users/me`) → **401** (a reset token must not authenticate).

### 1.4 🟢 Change password (logged in)
- **Reach:** Settings panel → Change password (or `PATCH /api/users/me/password`).
- **Steps:** enter current + new password → submit.
- **Expected:** 200; logging in with the old password fails, new password works; other sessions revoked.
- 🟠 **Edge:** wrong current password → 401/error, password unchanged.

---

## 2. Instance admin

### 2.1 🟢 Admin dashboard
- **Reach:** log in as the instance admin → admin area (InstanceAdminPage / `/admin`).
- **Steps:** view users list + instance stats (user/workspace/message/file counts).
- **Expected:** counts render; user list paginates.
- 🔴 **Edge — non-admin:** log in as a normal user and navigate to the admin URL directly → blocked (403 / redirected). Hit `/api/admin/users` without admin → 403.

### 2.2 🟠 Suspend user
- **Steps:** suspend a user → that user can no longer log in / act.
- **Expected:** suspended user's requests rejected; un-suspend restores access.

---

## 3. Workspaces

### 3.1 🟢 Create / switch / settings
- **Reach:** workspace sidebar → "+" to create; click a workspace to switch.
- **Steps:** create a workspace → it appears with a default `#general` channel and you as Owner.
- **Expected:** atomic — you're never left with a workspace that has no owner or no #general (transaction).
- 🟠 **Edge:** create with an empty/whitespace name → validation error.

### 3.2 🟢 Members: list / change role / remove
- **Reach:** Members panel.
- **Steps:** change a member's role (Member↔Admin), remove a member.
- **Expected:** UI updates live; removed member loses access.
- 🔴 **Edge — privilege:** as a plain Member, try to change roles / remove members → blocked (403). As Admin, you cannot elevate someone above your own level or remove the Owner.

### 3.3 🟢 Invites: create (email + shareable link), accept, revoke
- **Steps:** create an email invite (§1.2) AND a no-email "shareable link" invite with `max_uses`. Accept the link as an already-registered user. Then **revoke** an outstanding invite.
- **Expected:** link invite respects `max_uses`; revoked invite no longer works.
- 🔴 **Edge — max_uses race:** set `max_uses=1`, then accept the same link from two users near-simultaneously (two browsers, click together) → exactly **one** succeeds, the other gets "reached max uses". (This was a check-then-increment race; now atomic.)

### 3.4 🟠 Soft-delete + restore
- **Steps:** delete a workspace → it disappears for members; as Owner/Admin you see it in a "deleted" view → restore it.
- **Expected:** delete broadcasts to connected members live; restore brings it back. Members (non-admin) are moved out on delete.

---

## 4. Channels

### 4.1 🟢 Create public / private; join / leave
- **Reach:** channel sidebar → "+".
- **Steps:** create a public channel and a private channel; join/leave the public one.
- **Expected:** public channels are visible/joinable by any workspace member; private channels only to invited channel members.
- 🔴 **Edge — private access:** as a non-member, try to open a private channel / fetch its messages by id (`/api/channels/<id>/...` or `/api/messages/...`) → **403/404**, no message leakage.

### 4.2 🟠 Channel admin / archive
- **Steps:** channel admin renames/archives a channel; non-admin cannot.
- **Expected:** archived channel is read-only/hidden; permission enforced.

---

## 5. Messaging (the core)

### 5.1 🟢 Send / edit / delete
- **Reach:** open a channel, type in the composer.
- **Steps:** send a message; edit it; delete it.
- **Expected:** appears instantly for you and (live) for others in the channel; edit shows "edited"; delete removes it for everyone.
- 🟠 **Edge — soft delete:** after deleting, the message can't be edited/reacted/replied (operations on it 404). It also disappears from search.
- 🟠 **Edge — empty / huge:** empty message → not sendable; >4000 chars → validation error.
- 🟠 **Edge — idempotent send:** flaky network double-submit (same client id) → only one message persists (no duplicate).

### 5.2 🟢 Threads / replies
- **Steps:** open a message → reply in thread → send a few replies.
- **Expected:** the parent shows the correct **reply count** (incremented atomically); thread panel lists replies in order.
- 🟠 **Edge:** reply, then delete the reply → parent reply count stays consistent.

### 5.3 🟢 Reactions
- **Steps:** add an emoji reaction; add the same one again; remove it.
- **Expected:** reactions aggregate with counts; live to others; you can't double-count the same emoji.

### 5.4 🟢 Pins
- **Steps:** pin a message → open the Pinned panel → unpin.
- **Expected:** pinned set updates live.

### 5.5 🟢 Mentions (@)
- **Reach:** type `@` in the composer → mention dropdown.
- **Steps:** mention a user who is NOT currently viewing the channel.
- **Expected:** the message renders the mention highlighted; the mentioned user gets a **notification** (see §8) and a live push.
- 🟠 **Edge:** mention yourself → no self-notification. Mention in a private channel a user who isn't a member → they shouldn't receive channel content.

### 5.6 🟢 Rich text formatting
- **Steps:** use the formatting toolbar — bold, italic, underline, strike, code, code block, lists, quote; send.
- **Expected:** formatting renders in the message and round-trips on edit.
- 🔴 **Edge — XSS:** send a message containing `<img src=x onerror=alert(1)>`, `<script>alert(1)</script>`, and a `[click](javascript:alert(1))` markdown link.
  - **Expected:** rendered as inert text/markdown — **no alert fires**, no clickable `javascript:`/`data:` link, no raw HTML injected. (tiptap renders through a schema; raw HTML is stripped.)

### 5.7 🟢 Search
- **Reach:** Search panel.
- **Steps:** search for a word present in messages; filter by channel/user if available.
- **Expected:** full-text results ranked by relevance; deleted messages excluded; only workspaces/channels you can see.

### 5.8 🟠 Read tracking / unread badges
- **Steps:** in tab A send to a channel; in tab B (different user) don't open it.
- **Expected:** tab B shows an unread indicator for that channel; opening it clears the badge.

---

## 6. Files

### 6.1 🟢 Upload / download / delete
- **Reach:** composer → attach (paperclip) or drag-drop.
- **Steps:** upload an image and a PDF; download both; delete one (only the uploader can delete).
- **Expected:** files attach to the workspace; download returns the right content.
- 🔴 **Edge — only-own-delete:** try to delete someone else's file (via API `DELETE /api/files/<id>`) → 403.
- 🔴 **Edge — cross-tenant:** copy a file download URL and try it while logged in as a user NOT in that workspace → 403.
- 🟠 **Edge — size limit:** upload a file >100 MB → rejected (413/400), not a crash/OOM. (nginx `client_max_body_size 100m` + per-file cap.)
- 🔴 **Edge — content sniffing / stored XSS:** upload an `.html` file containing `<script>` and then open its download URL directly in the browser → it must download as an **attachment** (`Content-Disposition: attachment`, `X-Content-Type-Options: nosniff`), NOT render/execute.
- 🔴 **Edge — path traversal:** upload a file whose filename is `../../evil.txt` (use a tool/curl to set the multipart filename) → the server sanitizes it; nothing is written outside the data dir. (API-level: the upload still succeeds with a safe stored name.)

### How to send a raw upload for the edge cases (curl)
```bash
# get a token by logging in via the UI (devtools → copy access_token) or:
TOKEN=$(curl -s -X POST localhost:3000/api/auth/login -H 'Content-Type: application/json' \
  -d '{"email":"admin@dev.local","password":"<ADMIN_PASSWORD>"}' | jq -r .access_token)
# traversal filename:
curl -s -X POST "localhost:3000/api/files/upload/<WORKSPACE_ID>" -H "Authorization: Bearer $TOKEN" \
  -F 'file=@./payload.html;filename=../../evil.html;type=text/html' | jq
```

---

## 7. Direct messages (DMs)

### 7.1 🟢 Start / send a DM
- **Reach:** click a user → "Message" (DmView).
- **Steps:** send a DM both ways (A→B, B→A).
- **Expected:** both messages appear in the same conversation for both users (symmetric/deduped); delivered live.
- 🟠 **Edge:** delete a DM → excluded from history.
- 🔴 **Edge:** try to read another pair's DM conversation by manipulating the API user id → 403/empty.

---

## 8. Notifications

### 8.1 🟢 Mention notification + unread badge
- **Steps:** user A @-mentions user B while B is offline or viewing another channel → B opens the app / Notifications panel.
- **Expected:** B sees a **persisted** notification ("You were mentioned") with a deep link; the unread count reflects it; marking read clears it (and it stays cleared after refresh — it's persisted in the DB, not just a live push).
- 🟠 **Edge:** open the deep link → jumps to the message.

---

## 9. Realtime (presence, typing, live delivery)

> Best tested with **two browsers / two users**, and for scaling, **two realtime
> nodes** (run a second `chat-realtime` on another port behind the proxy).

### 9.1 🟢 Presence (online/offline)
- **Steps:** user A and B both online → A sees B as online → B closes the tab/logs out → A sees B go offline within ~60s (TTL) or immediately on clean disconnect.
- 🟠 **Edge — multi-tab:** B opens two tabs, closes one → B stays **online** (other tab/connection holds presence). Close the last → goes offline. (Presence is keyed per connection/node, so one tab closing doesn't flip you offline.)

### 9.2 🟢 Typing indicators
- **Steps:** A types in a channel → B (viewing it) sees "A is typing…" → stops → indicator clears.
- 🔴 **Edge:** typing should only reach members of that channel (not a non-member).

### 9.3 🟢 Live message delivery + ordering
- **Steps:** A sends several messages quickly → B sees them in the **correct order**, no gaps/dupes.
- 🟠 **Edge — reconnect:** kill the realtime connection (stop `chat-realtime` or toggle network) → the client shows "disconnected" and reconnects with backoff; after reconnect, new messages flow again.
- 🔴 **Edge — WS authz:** as user B, try to subscribe to a channel you're NOT a member of (craft a WS `channel.join` with another channel's id via devtools console) → you receive **no** messages for it.
- 🔴 **Edge — WS token type:** connect the websocket using a reset/refresh token in the `access_token` cookie → connection refused (only access tokens open a socket).

---

## 10. Webhooks & reminders (admin integrations)

### 10.1 🟢 Outgoing webhook
- **Reach:** workspace Admin → Integrations/Hooks (or `POST /api/workspaces/<id>/hooks`).
- **Steps:** create an outgoing webhook pointing at a request-bin you control (e.g. `https://webhook.site/...`) → post a message in the workspace.
- **Expected:** the bin receives the event with an `X-ChatSystems-Signature: sha256=…` HMAC header; the execution is logged.
- 🔴 **Edge — SSRF:** create a webhook pointing at an internal address: `http://127.0.0.1:3000/...`, `http://169.254.169.254/latest/meta-data/` (cloud metadata), `http://10.0.0.5/`, or an IPv6 ULA → dispatch is **rejected/blocked**, the request is never made to the internal target.
- 🔴 **Edge — authz:** as a non-admin, try to create/list/read/delete hooks (`/api/workspaces/<id>/hooks`) → 403; secret fields are redacted (`***`) on read.

### 10.2 🟢 Reminders
- **Steps:** create a reminder for yourself → it fires around the due time (delivered as a notification).
- 🔴 **Edge:** try to create a reminder targeting another user as a non-admin → 403.

---

## 11. Cross-cutting security & resilience (mostly API-level)

| # | Case | How to reach | Expected |
|---|---|---|---|
| 11.1 🔴 | Cross-tenant read | log in as B, hit `GET /api/workspaces/<A's id>/...` (members, channels, hooks, files) | **403** everywhere |
| 11.2 🔴 | Login brute-force | POST `/api/auth/login` wrong pw >10× for one email within 15 min | **429 Too Many Requests** |
| 11.3 🟠 | Redis down (fail-open) | stop the `redis` container, then log in with correct creds | login still **succeeds** (rate-limit fails open, doesn't lock everyone out) |
| 11.4 🔴 | Error leakage | trigger a 500 (e.g. stop Postgres mid-request) | response body is generic `{"error":"internal server error"}` — **no SQL/internal detail**; detail only in server logs |
| 11.5 🟠 | Graceful shutdown | `docker stop` the api with an in-flight request | request drains (not abruptly cut); `/readyz` flips to 503 when DB/Redis down |
| 11.6 🟠 | Body limit | POST a >100 MB JSON/file | rejected, not OOM |
| 11.7 🔴 | Missing/invalid token | call any `/api/...` (non-auth) without a token, or with a garbage token | **401**, flat "invalid or expired token" (no library internals) |

---

## 12. Known limitations to verify *don't* regress (not bugs)
- Presence is eventually-consistent across nodes (≤60s TTL on hard node crash) — a clean disconnect is immediate.
- Webhook dispatch validates DNS at request time (DNS-rebinding is out of scope); redirects are disabled.
- Search is Postgres full-text (English) — stemming/relevance, not fuzzy.

---

### Quick triage if something fails
1. `docker compose ps` — all healthy? `/readyz` green?
2. `docker compose logs api realtime` — JSON logs; find the `request_id` from the response and grep it.
3. Postgres reachable on 5433 (dev) / inside-network 5432; Redis on 6380 (dev) / 6379.
