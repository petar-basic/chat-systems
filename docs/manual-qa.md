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
#   JWT_SIGNING_KEYS and JWT_PUBLIC_KEYS (the commands are in .env.example)
#   SECRETS_KEYS=k1:$(openssl rand -base64 32)
#   ADMIN_PASSWORD=<something strong>
#   POSTGRES_PASSWORD=<something strong>
docker compose --profile frontend up -d --build
# App:      http://localhost:8080   (frontend nginx; proxies /api + /ws internally)
# API:      http://localhost:3000   (chat-api, loopback-only; health/metrics live here, NOT behind nginx)
# Realtime: http://localhost:3004   (chat-realtime, loopback-only)
# MailHog:  http://localhost:8025   (catches all outgoing email)
```
- First admin: `admin@dev.local` / `<ADMIN_PASSWORD from .env>`.
- Health: `curl localhost:3000/livez` → `ok`; `curl localhost:3000/readyz` → `ready`. Realtime too: `curl localhost:3004/livez` / `curl localhost:3004/readyz`. (`/livez`,`/readyz`,`/metrics` are served at the service root, not under `/api`, so hit the api/realtime ports directly.)
- Metrics: `curl localhost:3000/metrics` and `curl localhost:3004/metrics` → Prometheus text (api exposes `http_requests_total`/`http_request_duration_seconds`; realtime exposes `realtime_ws_connections`/`realtime_consumer_heartbeat_age_seconds`/`realtime_events_total`/`realtime_backpressure_drops_total`).

### Option B — dev mode (hot reload, two API ports)
```bash
# infra only
JWT_SIGNING_KEYS=dev:MC4CAQAwBQYDK2VwBCIEIGI30Lx2rSsmmyhD7LBOZcQoxLcgnMDiUmMNTIEro7Kk JWT_PUBLIC_KEYS=dev:MCowBQYDK2VwAyEApxzjT5NaCoM2MEpVGExZHIEbu4Q9d860CyDsO1lqJQs= SECRETS_KEYS=dev:ZGV2LXNlY3JldHMta2V5LTAxMjM0NTY3ODlhYmNkZWY= ADMIN_PASSWORD=admin123456 POSTGRES_PASSWORD=devpassword \
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

### 1.0 🔴 First run from /setup
- **Reach:** a stack with no instance admin: `ADMIN_PASSWORD` left empty on a fresh database, or `UPDATE users SET is_instance_admin = FALSE` and `docker compose restart api` on the dev stack (put the flags back afterwards).
- **Steps:** `docker compose logs api | grep "setup token"` → open `/` → it moves to `/setup` → enter a wrong token, then the right one, your name, email, a password and a workspace name → *Set up this instance* → *Send a test email* → *Open the workspace*.
- **Expected:** the API prints the same token on every start until setup is done. The wrong token says so and keeps what you typed. The right one signs you in as instance admin; the test email says *Sent to <you>* and arrives in MailHog. The workspace opens on #general. `/setup` afterwards says *This instance is already set up*, and the same token answers 409.
- 🔴 **Edge — SMTP problems in words:** set `SMTP_PORT=1` and restart api, then *Instance admin → Email → Send a test email* → *Nothing accepted the connection at mailhog:1…*. `SMTP_TLS_MODE=implicit` against MailHog names the TLS mode instead.

### 1.0b 🔴 A new workspace's checklist and an empty channel
- **Reach:** right after 1.0, or as Admin create a workspace from the workspace rail.
- **Steps:** look above the messages in `#general` → *Invite people* and send an invite → *Create* a channel → *Choose* an icon in *General* → follow *How to* for SSO → dismiss with ×, then reload. In a new empty channel use *Add people* and *Add a bookmark*.
- **Expected:** *Get <workspace> going* shows *0 of 4 done* (*1 of 4* where SSO is configured); each step ticks as soon as it is done, from wherever it was done. The link opens the RUNBOOK's *Single sign-on*. Dismissed, it stays gone after the reload, and only for that workspace. `#general` empty says *Everyone in the workspace is here…*; a channel with a description shows it. *Add people* in `#general` (or while you are the workspace's only member) opens the invite form, elsewhere the channel's member list already adding; *Add a bookmark* opens the bookmark form in the bar.
- 🟠 **Edge — who:** Bob (member) sees no checklist, and no *Add a bookmark* in a channel he does not moderate. The dev workspace (many members and channels) shows no checklist to anyone.

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
- 🔴 **Edge — no email:** with `SMTP_HOST=` empty, invite somebody → the form says email is not set up and shows the link with a copy button (with mail configured but the outbox failing it says the email could not be sent instead); nothing reaches MailHog. The copied link registers them as above. Under *Members* the invite sits in *pending invites* until used; its link icon copies a working link, and *Revoke* (confirmed) removes the row and kills every copy of the link.
- 🔴 **Edge — SSO from the invite:** with SSO configured, invite `sso@dev.local` to a workspace it is not in and open the link in a private window → *Continue with SSO* → you land inside that workspace, signed in.
- 🟠 **Edge — forgot password:** *Forgot password* for an invited address that never registered → MailHog gets the invite again, not a reset. For `sso@dev.local` → a note to use *Sign in with SSO*. On a reset link, a 5-character password is refused and the same link then works with a proper one.
- 🟠 **Edge — password too short:** enter <8 chars → validation error, no submit.
- 🔴 **Edge — mixed case (CS-094):** invite `Mixed.Case@Dev.Local` → register from the link → sign in as `mixed.case@dev.local` and as `MIXED.CASE@DEV.LOCAL` → both work, and *Admin → Users* lists one lowercase account. With SSO, invite `SSO@dev.local` and sign in through the provider → the invited account, not a second one.

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

### 2.2 🟠 Suspend user → session revocation
- **Reach:** admin area → user row → Suspend (or `POST /api/admin/users/<user_id>/suspend`).
- **Steps:** with the target user logged in and on a live socket, suspend them.
- **Expected:** their existing **access token is rejected (401 "Session revoked")** on the next `/api/...` call — not just on next login — and their **live WebSocket drops** (driven by a `user.suspended` event); fresh login fails too ("Account is not active"). Their refresh tokens are deleted, so silent refresh can't resurrect the session.
- 🔴 **Edge — what they set up stops too:** before suspending, have them schedule a message, own an on workflow and an incoming webhook. After the suspend the scheduled message fails, the workflow is *Off* with a notification to the admins, and *Integrations* shows the webhook *Disabled — its creator's account is suspended*. Activating them again leaves both off. As a workspace admin open the workflow → *Take over* → it is yours and *Off*; *Turn on* works.
- 🟠 **Edge — activate restores:** `POST /api/admin/users/<user_id>/activate` clears the Redis revocation flag → the user can log in again and act. (Revocation is a Redis `revoked:<uid>` key TTL'd to the access-token lifetime; if Redis is unavailable the access token is *not* treated as revoked — fail-open.)

---

## 3. Workspaces

### 3.1 🟢 Create / switch / settings
- **Reach:** workspace sidebar → "+" to create; click a workspace to switch.
- **Steps:** create a workspace → it appears with a default `#general` channel and you as Owner.
- **Expected:** atomic — you're never left with a workspace that has no owner or no #general (transaction).
- 🟠 **Edge:** create with an empty/whitespace name → validation error.
- 🟢 **Rail:** create "Design" and "Dev Team" with no icon → the rail shows *De* and *DT* on tiles of different colours; hovering one shows its full name, and the open one has a ring.

### 3.1b 🟢 Workspace settings in one dialog
- **Reach:** workspace menu → *Workspace settings*.
- **Steps:** as Admin, go through every section; in *General* rename the workspace and click away, flip *Link previews*, set *Who can build workflows*. Open `/app/<ws id>?settings=audit-log`. As Bob (member), open *Workspace settings*.
- **Expected:** the menu holds *Invite people*, *Workflows*, *Workspace settings* (and *Instance admin* for one). *General* says *Saving…* then *Saved* after each change, with no save button; the sidebar shows the new name. The link opens the dialog on *Audit log* and the `?settings=` leaves the address. Bob sees *Members*, *User groups*, *Custom emoji* and *Apps* only, and no invite form. An icon Admin uploads in *General* shows in Bob's sidebar and switcher too; Admin replaces it, then *Remove* → the old URL answers 404 each time. Alice (admin, not owner) has no *Data retention*; only an instance admin sees *SCIM provisioning*.
- 🟠 **Edge — Escape:** in *Integrations*, *Delete* a webhook to open its confirmation, press Escape → only the confirmation closes; Escape again closes the dialog.

### 3.1c 🔴 Retention, exports, erasure and SCIM
- **Reach:** *Workspace settings* → *Data retention*, *Exports*, *SCIM provisioning*; *Members* rows.
- **Steps:** as Admin (owner and instance admin): in *Data retention* choose *For a number of days* for messages and type 30, wait, then *Save retention*; set it back to *Forever* and save. In *Exports* tick nothing and *Export workspace*, wait for *complete*, *Download*. In *Members* use the export icon on Bob's row, then the erase icon on a throwaway account's row. In *SCIM provisioning* create a token, *I have stored it*, rotate it, revoke it.
- **Expected:** the retention form says *About N messages and M files would be removed now* before anything is saved, and *Saved* after. The export row goes pending → running → complete (the worker polls every 10 s); the archive downloads once, after which the row says *Downloaded*, and the link opened again answers 404. Bob's export shows up under *Exports* with his name. Erase names what goes and stays disabled until the person's name is typed exactly; afterwards the row reads *Deleted user*. A token is visible only until it is put away; rotate shows the replacement the same way, and the revoked one is struck through with no buttons.
- 🔴 **Edge — what an export holds:** post a file in a DM, then *Export workspace* without DMs: `files.jsonl` in the tar lists no file from the DM. With *since* set, `message_edits.jsonl`, `reactions.jsonl`, `polls.jsonl` and `poll_votes.jsonl` hold only those of messages in the range. With two workspaces on a retention period, each one's *Retention purge* row in the audit log counts only its own messages.
- 🔴 **Edge — who:** Alice (admin) sees *Exports* but no *Export workspace* button, no *Download* on the owner's exports, and no export/erase icons; `GET /api/workspaces/<id>/retention/preview` answers her 403.

### 3.1d 🔴 Retention per channel
- **Reach:** *Workspace settings → Data retention*; a channel's ⋯ → *Channel details → Settings*.
- **Steps:** as Admin (owner), tick *Let channels keep messages for their own period* and save. In #random open *Channel details → Settings*, set *Message retention* to *A number of days*, 3650, *Save*; then *Keep forever*; then *Workspace default*. As Bob open #random's settings if he can (he cannot moderate). Untick the workspace option and reopen #random's settings.
- **Expected:** each change posts a line in #random from Admin (*set this channel to keep messages for 3650 days*, *…forever*, *…follow the workspace's message retention*) and an *Audit log* row. With the option off, *Settings* says *Messages here follow the workspace…* and has no control; `PUT /api/channels/<id>/retention` answers 409.
- 🔴 **Edge — who:** as Bob, `PUT /api/channels/<random id>/retention` with `{"message_days": 1}` answers 403.

### 3.2 🟢 Members: list / change role / remove
- **Reach:** workspace menu → *Workspace settings* → *Members* (an admin also has *Invite people*, which opens it with the invite form).
- **Steps:** as Owner/Admin, change a member's role from the row dropdown, then remove a member (Remove → confirm).
- **Expected:** UI updates live; removed member loses access. Rows you may not manage (yourself, the Owner, anyone at or above your level) show a read-only role label instead of controls.
- 🔴 **Edge — privilege:** as a plain Member or Guest there are no role controls at all, and the API rejects the calls (403). As Admin you cannot promote yourself or anyone to Owner, demote the Owner, or remove the Owner.
- 🔴 **Edge — deleting the workspace:** `DELETE /api/workspaces/<id>` is Owner-only (or instance admin); a workspace Admin gets 403.

### 3.2b 🔴 Guest scope
- **Reach:** log in as a Guest (`diana@dev.local` in the seed).
- **Expected:** the Guest sees only the channels they were explicitly added to — public channels are **not** implicitly readable (`GET /api/channels/<public_id>/messages` → 403) — and `POST /api/workspaces/<ws>/channels` → 403.
- 🟢 **Sanity:** after an Admin adds the Guest to a channel, reading/posting there works.

### 3.3 🟢 Invites: create (email + shareable link), accept, revoke
- **Steps:** create an email invite (§1.2) AND a no-email "shareable link" invite with `max_uses`. Accept the link as an already-registered user. Then **revoke** an outstanding invite.
- **Expected:** link invite respects `max_uses`; revoked invite no longer works — the emailed link of a revoked email invite included (the registration page says the invite is invalid).
- 🔴 **Edge — max_uses race:** set `max_uses=1`, then accept the same link from two users near-simultaneously (two browsers, click together) → exactly **one** succeeds, the other gets "reached max uses". (This was a check-then-increment race; now atomic.)

### 3.4 🟠 Soft-delete + restore
- **Steps:** delete a workspace → it disappears for members; as Owner/Admin you see it in a "deleted" view → restore it.
- **Expected:** delete broadcasts to connected members live; restore brings it back. Members (non-admin) are moved out on delete.
- 🟠 **Edge — instance-admin delete is also soft + reversible:** `DELETE /api/admin/workspaces/<id>` no longer hard-deletes — it sets `is_active=false`/`deleted_at` and writes the audit row in the **same transaction** (so the audit can't silently drop), and broadcasts `workspace.deleted`. The workspace can still be restored.

---

## 4. Channels

### 4.1 🟢 Create public / private; join / leave
- **Reach:** channel sidebar → "+".
- **Steps:** create a public channel and a private channel; join/leave the public one.
- **Expected:** public channels are visible/joinable by any workspace member; private channels only to invited channel members.
- 🔴 **Edge — private access:** as a non-member, try to open a private channel / fetch its messages by id (`/api/channels/<id>/...` or `/api/messages/...`) → **403/404**, no message leakage.
- 🔴 **Edge — who creates:** as Admin set *Workspace settings → General → Who can create public channels* to *Admins only*. Bob's *Create channel* offers only *Private*; set private to admins too and Bob has no *+* by *Channels* and no *Create channel* in *Browse channels*. `POST /api/workspaces/<id>/channels` as Bob answers **403** naming the kind. Put both back.

### 4.1b 🔴 Who may notify a whole channel
- **Reach:** *Workspace settings → General → Permissions*; a channel's ⋯ → *Channel details → Settings*.
- **Steps:** as Admin set *Who can use @channel, @here and @everyone* to *Admins only*. As Bob type `@` in #random, then send `@channel hello`. Make Bob a channel admin of #random and send it again. Put the rule back; in #random's *Settings* untick *Allow @channel, @here and @everyone* and send `@here` as Admin.
- **Expected:** Bob's suggestions have no `@channel`, `@here` or `@everyone`; his send fails with *Only workspace admins and this channel's admins can use @channel, @here and @everyone*. As a channel admin it goes through. With the channel switch off even the Admin is refused (*…turned off in this channel*), and *Audit log* has *Broadcast mentions changed*.
- 🟠 **Edge — large channel:** in a channel of more than 25 people a broadcast asks *This notifies N people. Send anyway?*; *Cancel* keeps the text in the composer.

### 4.2 🟠 Channel admin / archive
- **Steps:** channel admin renames/archives a channel from the header's ⋯ → *Channel details → About*; a non-admin sees the topic and description as text and no *Settings* tab.
- **Expected:** archived channel is read-only/hidden; permission enforced. A plain member's header has no ⋯ on desktop.
- 🔴 **Edge — archived stays closed:** with Bob viewing the channel in a second window, Admin archives it. Both see *archived this channel* and the composer turns into the *Archived* bar at once; Bob's bar has no *Unarchive*. *Browse channels* lists it with *Archived* and no *Join*. A scheduled message into it fails, and its incoming webhook answers 403. Admin's *Unarchive* brings the composer back for both, with *unarchived this channel*.
- 🔴 **Edge — read-only all the way:** in an archived channel, hovering a message shows no reactions, *Pin*, *Edit* or *Delete*; reaction pills and poll options do not respond; the header has no huddle button, *Channel members* has no *Add people*, *Remove* or admin toggle, and the bookmark bar no *Add a bookmark*. Archive a private channel: it leaves the sidebar, and *Browse channels* lists it last as *Archived private channel* for its members only; opening it shows *Unarchive* to its admins.
- 🟢 **Header:** the people count is the one members button and toggles the panel; the pin button shows how many are pinned. Every header icon has a tooltip.

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
- 🟠 **Edge:** reply, then delete the reply → the parent's count goes down by one, in the channel and in a second tab with the thread open, and the reply leaves that tab's thread at once. A reaction on a reply shows in the other tab's open thread without reopening it.
- 🟠 **Edge — long thread:** a thread past 50 replies opens on the newest; *Show earlier replies* loads the rest without jumping, back to the first.
- 🟠 **Edge — deleted parent:** delete a message that has replies → it stays as *This message was deleted* with its reply count; the thread still opens and takes replies. Deleting one with no replies removes it. No reply ever appears in the channel on its own. With the thread open, delete its last reply → the composer gives way to *This thread was deleted*, and a reply sent anyway (API) answers 404.
- 🟢 **Also send to #channel:** tick it under the thread composer and send → the reply shows in the thread and in the channel with *Replied to a thread*, which opens the thread on it; members on *All* are notified as for a channel message. Schedule a reply from the thread composer → it arrives in the thread (*Scheduled* labels it *· thread*). A card in *Threads* has the same *Also send to* box and *Schedule* button.

### 5.2a 🟢 Sending never loses work
- **Steps:** attach a file, write a message, turn the network off (DevTools → Offline) and send; switch channels and come back; turn the network on and press *Retry*. Repeat in a thread, and press *Discard* on one.
- **Expected:** the failed row keeps its text and file across the switch; *Retry* sends it with the file; *Discard* removes the row and puts the text and the file back in that composer.
- **Edge:** reply from a card in *Threads* with the network off → the reply shows on the card as *Failed to send* with *Retry* and *Discard*. Reload the page with a failed message in a channel → it is still there. Schedule a message whose tray has a file that failed to upload → *A file did not upload*, and nothing is scheduled.
- 🟠 **Edge — files per channel:** attach a file in #a, switch to #b and back → the file is still in #a's tray, and #b's tray is empty. A file whose upload failed says why and blocks *Send* until it is retried (click it) or removed.
- 🟠 **Edge — failed scheduled message:** schedule a message into a private channel, leave it, wait for the time → *Scheduled* shows *Not sent* with the reason, *Send now* and the bin; rejoin and *Send now* delivers it, the bin removes it.
- 🟠 **Edge — forward:** forward a message that says `@channel` and names someone → only people named in your comment are notified, and forwarding is allowed where you may not broadcast.

### 5.3 🟢 Reactions
- **Steps:** add an emoji reaction; add the same one again; remove it.
- **Expected:** reactions aggregate with counts; live to others; you can't double-count the same emoji.

### 5.4 🟢 Pins
- **Steps:** pin a message → open the Pinned panel from the header's pin button → unpin.
- **Expected:** pinned set updates live, and so does the count on the pin button.

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
- **Reach:** Search panel (`GET /api/search?workspace_id=<id>&q=…`, optional `channel_id`/`user_id`).
- **Steps:** search for a word present in messages; filter by channel/user if available.
- **Expected:** full-text results ranked by relevance; deleted messages excluded. Results are **access-scoped**: you must be a member of the queried workspace (else 403), and matches only come from **public channels + the private/DM channels you actually belong to**.
- 🔴 **Edge — private leakage:** as a user who is NOT a member of a private channel, search for a term you know is only in that private channel → **no results** from it. Search a workspace you're not in → **403**.
- 🟠 **Edge — more than twenty:** search a word in more than twenty messages → the count reads *20+*, *Show more* adds the next twenty, and it disappears when there are no more. The same on *Files*.
- 🟠 **Edge — where a hit opens:** click a hit in a DM → the DM opens on the message; a thread reply → the thread opens beside its channel, scrolled to the reply and highlighted; a public channel you have not joined → the hit is labelled with that channel's name, the channel opens on the message with a *Join channel* bar instead of the composer, and joining puts it in the sidebar.

### 5.8 🟠 Read tracking / unread badges
- **Steps:** in tab A send to a channel; in tab B (different user) don't open it.
- **Expected:** tab B shows an unread indicator for that channel; opening it clears the badge.
- 🟢 **Read on the server too:** open that channel from a link or by reloading on it, not from the sidebar, then reload again → it stays read, and a mention notification from it no longer counts in the bell.
- 🟢 **One rule (CS-082):** with B looking at the channel, A @-mentions B → no badge on the row, no unread Activity row, and none after B leaves the channel. A @-mentions B in a thread reply → no badge on the channel; opening the channel leaves the Activity row, opening the thread clears it. In a DM the same: the open DM leaves nothing behind.
- 🟢 **Other workspace (CS-082):** with B in workspace X, A @-mentions B in workspace Y → only Y's rail badge moves; X's sidebar is untouched. The app icon shows X + Y.
- 🟢 **First unread (CS-082):** A posts 40 messages to a channel B had read → B reloads straight onto it → the list opens on the first new message under the *New* line; scroll down a screen → *Jump to first unread* appears and takes B back; at the bottom it goes away.
- 🟢 **Read state (CS-092):** with a thread-only reply as a channel's newest message, *Mark all read* (Shift+Esc), then A posts once → opening it loads one page, the *New* line above A's message. A @-mentions B in a reply with *Also send to channel* → B opens the channel → the Activity row is read and no email follows. A deletes the root of a thread B follows → B's reply notification from it still opens the thread. A DM B has read, then two more from A → B opens it at the line above them. B's keyword quoted in a forward → no keyword alert.
- 🟢 **Request count (CS-072):** in the browser's network tab, switch to a channel not opened before → two `/api` requests, `channels/<id>/context` and `channels/<id>/messages?…&mark_read=true`; switch back within half a minute → only the messages.

### 5.9 🟢 Polls
- **Reach:** the poll button next to the paperclip, or `/poll` in any message box (`POST /api/channels/<id>/polls`).
- **Steps:** create a poll from the dialog with three options, reorder one by dragging and one with Alt + ↑; send `/poll "Lunch?" "Thursday" "Friday"` in a DM and in a thread. In tab A vote, in tab B (another user) vote for something else; press your chosen option again; switch a single-choice vote; on a multiple-choice poll pick two.
- **Expected:** each poll appears where it was made; counts and bars move at once in your tab and arrive live in the other; your choice has a check; avatars of who voted show on each option; pressing a chosen option takes the vote back. Focus the poll message and press 2: the second option is voted. The footer reads "N votes", "· closes …" when a closing time was set.
- 🔴 **Edge — anonymous:** on an anonymous poll no avatars show, and `GET /api/polls/<id>/votes` answers 403 even for its creator.
- 🟠 **Edge — closing:** *Close poll* shows for the creator only; once closed the rows are disabled and the footer reads "Closed". A poll with a closing time a minute away closes itself (worker reminder loop) and refuses a vote with 409. A poll message has no *Edit*.

### 5.10 🟢 Lists, the table
- **Reach:** *Lists* at the start of the channel's bookmark bar → *New list* (`POST /api/workspaces/<id>/lists`); a list lives at `/app/<workspace>/lists/<list>`.
- **Steps:** create a *Project tracker*. *New item*, type a task, Enter. Without the mouse: arrows to *Status*, Enter, pick *In progress*; type `b` on *Owner* and pick a person; Enter on *Due* and pick a date; Tab across a row; Escape out of an edit; Delete a cell; Alt + ↓ a row; Shift + Enter to open it. In tab B (another member) open the same list and edit a different cell of the same row.
- **Expected:** every edit shows at once and arrives live in the other tab; both people's edits to one row stay. The item panel shows every field and edits them; ↑ from the first row lands on the headers and Enter opens the column menu.
- 🟠 **Edge — columns:** add a *Select* column with three choices; rename one; change a filled *Number* column to *Date* → the dialog says how many cells it clears and saves only on the second press. Hide a column → *1 hidden* in the header shows it again; the item panel still lists it. Move a column left and right; delete one.
- 🟠 **Edge — size:** with 10,000 items (`INSERT … generate_series` as in `http_tests/lists.rs`) the table scrolls smoothly and Ctrl + End reaches the last row; *New item* is disabled at the limit.
- 🔴 **Edge — read-only:** in an announcement channel a member who cannot post sees the list but no *New item*, no editors and no column menu; `PATCH /api/list-items/<id>` answers 403.
- 🟠 **Edge — drag:** drag a row by its grip above another; the line shows where it lands; reload and the order holds.

### 5.11 🟢 Lists, views
- **Reach:** the tabs under a list's title; *+* → *Board*.
- **Steps:** on a *Project tracker* with a few items, add a *Board*. Drag a card from *Not started* to *Done*; on another card press Alt + → twice. *New* in a lane, type a title in the panel. *Sort* → *Due*, *Descending*; *Filter* → *Only mine*, then *Due this week*. Rename the view from its menu; add a second table and move it left. In tab B (another member) open the same list.
- **Expected:** tab B has the same tabs, the board shows the same lanes and moves arrive live; the filter count shows on *Filter*, and *Only mine* shows each person their own items. The header says *N of M items* while a filter hides some.
- 🟠 **Edge — columns go:** filter a board by a choice, then delete that choice, then delete the *Status* column → the filter goes, then the board asks for a column to group by; no error.
- 🟠 **Edge — sorted table:** sort a table → no row grips, Alt + ↓ does nothing; *New item* adds a row that stays visible even if the filter would hide it, until reload.
- 🔴 **Edge — read-only:** in an announcement channel a member who cannot post sees the views and their rules but cannot change them or drag cards; `POST /api/lists/<id>/views` answers 403. Deleting the last view is refused (422).

### 5.12 🟢 Lists, comments, assignment and due dates
- **Reach:** open an item (Shift + Enter, or the row's open button); *Comments* is under its fields. The *Due* column's menu → *Use as due date*.
- **Steps:** as Ana, on a *Project tracker* in a channel Bob is in, mark *Due* as the due date. Set an item's *Owner* to Bob and its *Due* to tomorrow. Comment on it with `@Bob`. Set another item's *Owner* to yourself.
- **Expected:** Bob's Activity has "Ana assigned you to *…*" and "Ana mentioned you on *…*"; clicking either opens the list with the item. Nothing for the item Ana assigned to herself. Bob sees the comment arrive in the open item and the row's comment count. Bob's Later has the item with the list's title, *Due tomorrow 9:00*; ticking it off checks the item's *Done* for everyone.
- 🟠 **Edge — handed on:** reassign the item to Carol → it leaves Bob's Later and appears in Carol's; move its date a week out → it leaves Later until it is due tomorrow again (the reminder checker catches a day passing within ten minutes).
- 🟠 **Edge — mentions:** mention someone who is not in the channel → no notification for them. Mark *Name* (text) as the due date → 422.
- 🔴 **Edge — comments:** Bob cannot edit Ana's comment (`PATCH /api/list-comments/<id>` 403) nor delete it; a channel moderator can delete it; a member outside a private channel gets 403 on `GET /api/list-items/<id>/comments`.

### 5.13 🟢 Workflows
- **Reach:** workspace menu → *Workflows* → *New* → *Blank* (or a template) → *Continue*.
- **Steps:** as Bob, build "Alert relay": trigger *When a webhook is called*, variable `service`; step *Send a message* to #general with `{{webhook.service}} is down` (use *Insert variable*). *Create workflow*. Copy the URL and run `curl -X POST <url> -H 'content-type: application/json' -d '{"service":"billing"}'`. Build a second one on a schedule a minute or two from now, *Create workflow*.
- **Expected:** curl answers 202; within seconds "billing is down" appears in #general from *Alert relay*, tagged *Workflow*. *Run history* shows *Succeeded* with *Step 1 · Send a message*. The scheduled one posts at its minute, once. Admin's panel lists both with "Built by Bob".
- 🟠 **Edge — retries:** send the same curl twice with `-H 'Idempotency-Key: x'` → one message. *Turn off* the workflow and call it → 409; *Turn on* → 202 again. There is no *Save draft*: a new workflow runs as soon as it is created. 31 calls in a minute → 429.
- 🟠 **Edge — authority:** a step to a private channel, then remove Bob from it and call the webhook → *Failed*, "Step 1 (Send a message): Its creator can no longer post in that channel". Remove Bob from the workspace → his workflows turn off and the admins' Activity says so.
- 🔴 **Edge — the link moves on:** open "Alert relay" → *New URL* → the old URL answers 401, the new one 202, and the audit log has *Workflow URL replaced*. Suspend Bob, *Take over* as Admin → the URL changes again and the one Bob knew answers 401.
- 🔴 **Edge — who builds:** *Workspace settings → General* → *Who can build workflows* → *Admins only* → Bob's *New* is gone and `POST /api/workspaces/<id>/workflows` answers 403; a guest never sees *Workflows*.
- 🟢 **Event triggers:** as Bob, build "Welcome": *When someone joins a channel* → #random; step *Send a DM* to *The person who joined*, "Welcome {{trigger.user}} to {{trigger.channel}}". Build "Incident": *When a reaction is added* in #general, emoji 🚨; step *Send a message* to #random with "{{trigger.user}} flagged {{trigger.message_link}}". Create both. Ana joins #random, Ana adds 🚨 to a message in #general.
- **Expected:** within seconds Ana has a DM from *Welcome*, tagged *Workflow*, mentioning her; #random gets the flag with a link that opens the message. Ana joining again, 👍 instead of 🚨, or 🚨 in another channel starts nothing. Bob joining #random himself is a succeeded run that sent no DM.
- 🟠 **Edge — private channel:** a join trigger on a private channel Bob isn't in → "You can't see the channel it listens to". Add Bob, create it, remove Bob, add someone → the run fails with "Its creator can no longer see the channel it listens to".
- 🟢 **Shortcuts and forms:** as Bob, build "Time off": *From a shortcut in a channel* → #general; step 1 *Ask a person to fill in a form* to *The person who ran it*, title "Request time off", questions *Reason* (long answer, required), *Kind* (choice: Sick, Holiday), *Cover* (person), *From* (date); step 2 *Send a message* to #general with "{{trigger.user}} is off from {{form.from}}", *Add the form answers below the message* ticked. *Create workflow*. As Ana, open #general → the ⚡ button in the composer → *Time off*.
- **Expected:** a toast says it started; Ana's DM with Bob gets a *Request time off* card with *Fill in*. Submit empty → "“Reason” needs an answer". Fill it in, picking Admin as cover → the card says *Filled in*; within seconds #general has Ana's message with the answers laid out as fields in two columns (one on a phone, the long *Reason* across both), and Admin is notified as cover. *Run history* shows *Succeeded* with both steps. `/workflow time off` in #general starts it too; `/workflow` alone lists the shortcuts.
- 🟠 **Edge — forms:** Bob sees "Waiting for Ana" on a card that isn't his and no button; a second submit → 409. Running his own shortcut, Bob's form appears in #general with *Fill in* only for him. Edit the workflow while a form waits, then submit → *Failed*, "The workflow was edited while this run was under way". A form left alone for 7 days → *Expired* in the history, "This form expired" on the card.
- 🟢 **Lists:** create a list in #general with *Kind* (select: Holiday, Sick, Other), *From* and *Until* (date), *Who* (person) and *Status* (select: Open, Approved). As Bob, *New* → *Time-off request into a list* → *Continue*: the form, *Add an item to a list* and the message are filled in. Pick the list, *Fill from the form answers*, set *Who* to *Person who ran it*. Add a step *Update the item a step added* → step 2, *Status* → *Approved*. *Create workflow*, then run it as Ana from the ⚡ button and fill in the form.
- **Expected:** the list gets a row with Ana's kind, dates and Ana as *Who*, marked *Approved*; Ana is notified she was assigned. *Run history* shows every step *Succeeded*.
- 🟠 **Edge — list steps:** a *Kind* question whose choices include one the column lacks → saving says "“Kind” has no choice “…”, which {{form.kind}} can answer". Add a *Days* number column filled from a short-answer question, answer "three" → *Failed*, "“Days” needs a number, not “three”". Put a second form between the two list steps and delete the item before answering it → "The item it updates was deleted". Remove the add step → the update step asks for an earlier step again.

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

### 6.2 🟢 Finding a file by name
- **Steps:** post `Q3 budget.xlsx` with no text. Open search → *Files*, type `budg`.
- **Expected:** the file is listed with a spreadsheet icon, `budg` marked in its name, its size, uploader, channel and date. Clicking it jumps to its message; an image opens the gallery; the download icon appears on hover. `type:pdf` in the box switches to *Files* and sets *Type*.
- 🟢 **Channel panel:** the channel's *Files* panel → *Filter by name* `budg` → only matching files, and the kind tabs still narrow them.
- 🔴 **Edge — visibility:** as a member outside a private channel, or a guest outside a public one, a file posted there is not found; neither is a file whose message was deleted, or an upload never sent.

### 6.3 🟢 Finding a file by what is inside it
- **Steps:** post a PDF, a Word document, a spreadsheet and a slide deck, each with a word that is not in its name (a client's surname, a city). Wait up to a minute for the worker, then search *Files* for that word, typed without its accents.
- **Expected:** each file is listed under any name match, with a line of its text under the name and the word marked in its original spelling. `file_text_extraction_total{status="done"}` on the worker's `/metrics` went up by four.
- 🟠 **Edge — unreadable files:** a password-protected PDF, a truncated PDF and a renamed `.exe` → no results and no worker restart; `file_text_extraction_total` counts them as `unsupported` / `failed`, once each.
- 🔴 **Edge — markup in a file:** a `.txt` containing `<img src=x onerror=alert(1)>` near the searched word → the snippet shows the characters as text; nothing runs.
- 🔴 **Edge — visibility:** the same word in a file in a private channel you are not in → not found.

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
- 🟠 **Edge:** open the deep link → jumps to the message. From *Activity*, a desktop notification and a phone push alike: a DM opens the conversation, a reply opens its thread scrolled to it, and an old `/app/<ws>/<channel>/<message>` link to a DM or a reply lands the same way.
- 🟠 **Edge — gone:** open the link of a message since deleted, or of a private channel you were removed from → *That message was deleted or you can no longer see it*, and the channel (or the workspace) opens instead.
- 🔴 **Edge — workspace membership:** the workspace-scoped notifications routes now require membership. As a user NOT in workspace W, call `GET /api/workspaces/<W>/notifications` (also `.../notifications/unread-count`, `.../notifications/read-all`, `.../channels/<id>/notifications/read`) → **403** "Not a member of this workspace".

### 8.2 🟢 Notification levels
- **Steps:** as B, open a channel's bell (header, or the sidebar row on hover) → *Nothing*. A @-mentions B there while B is in another channel.
- **Expected:** the sidebar row shows a small crossed-out bell; the mention badge still appears; no sound, no desktop notification, no Activity row, no push. Back to *Default (Mentions)* → the next mention notifies as in 8.1.
- 🟢 **All new messages:** set a channel to *All* → a plain message from A rings and reaches B's phone, but adds nothing to Activity. A thread reply does not ring unless B follows the thread.
- 🟢 **Default:** *Preferences → Notifications → Notify me about* → *All* changes every channel left on *Default*; one with its own level keeps it.
- 🟢 **DMs:** A sends B a DM without an `@` while B has no session open → B's phone gets it (title = A's name) and Activity has a DM row; with no push device it is in the next email digest as "a direct message". Under DND only the Activity row appears.
- 🟠 **Edge:** a mention of somebody who is not in the channel notifies nobody. `PUT /api/channels/<id>/notify-level` as a non-member → **403**.

### 8.3 🟢 Keyword notifications
- **Steps:** as B, *Preferences → Notifications → My keywords* → type `deploy`, Enter (and `c++`, comma). A writes "Deploy!" in a channel B is in, while B is elsewhere or offline.
- **Expected:** one Activity row "Matched "deploy" in #channel" with a tag icon, a push when B has no session, a desktop notification when B has one. The word is highlighted in the message for B only. The sidebar mention badge does not change (intended).
- 🟢 **Matching:** "deploý" notifies; "redeployed" does not; "C++" matches `c++` literally.
- 🟢 **One per message:** "@B can you deploy?" gives one mention, not a mention and a keyword.
- 🟠 **Edge:** a channel on *Nothing* or muted never notifies for keywords; nor does a private channel B is not in, nor A's own keyword in A's message. `Deplóy` next to `deploy` is refused as a duplicate; a 51st keyword, `a`, `@ana` and `#general` are refused.

### 8.4 🟢 Notification schedule
- **Steps:** as B, *Preferences → Notifications → Notification schedule* on, pick days and *From*/*To* so that now is outside the window. A mentions B.
- **Expected:** the mention lands in Activity, but no push, sound, desktop notification or email. The *You* menu and the Activity panel's moon say "Paused by your schedule until …"; A's hover card on B says "Notifications paused until …".
- 🟢 **Resume:** *Resume notifications* during the pause → mentions notify again until the next window starts; after that window ends the schedule pauses again.
- 🟢 **Night window:** *From* 22:00 *To* 07:00 — notifications on at 23:30 and 06:30, paused at noon.
- 🟠 **Edge:** the hours follow the timezone in *Edit profile*; changing it moves the window. `PUT /api/workspaces/{id}/notifications/schedule` with start = end, day 0 or 8, or no days → **422**.

### 8.5 🟢 Notifications once (CS-083)
- 🟢 **Email:** B has mention emails on, no app open. A @-mentions B → five minutes later (mailhog) one email, "You were mentioned", whose link opens the message. A @-mentions B again and B opens the channel within five minutes → no email. Redelivering the event (restart `worker` mid-batch) still gives one email.
- 🟢 **Idle tab:** B has push on and the app open in a background tab. A @-mentions B → B's push arrives. B brings the tab to the front and moves the mouse; A mentions again → no push, the in-app notification only. Leave it untouched for two minutes → push again.
- 🟢 **Shut laptop:** B uses the app on a laptop and has push on a phone. B shuts the lid mid-use; A @-mentions B 40 seconds later → the phone gets it.
- 🟢 **Badge after leaving:** B has unread mentions in workspaces X and Y and leaves Y. A mentions B in X with B's app closed → the phone's badge equals the app's badge.
- 🟢 **Second server:** B is signed in to two instances, viewing the first. A mentions B on the second with another app in front → clicking the desktop notification opens the message on the second instance.
- 🟢 **Ring under pause:** B pauses workspace X (*You* menu → *Pause notifications in X* → *For 1 hour*). A starts a DM huddle with B in X → B's app does not ring; Activity shows the call. Resume → the next ring rings.
- 🔴 **Guest call:** a guest starts a DM huddle (API, `dm_partner_id`) with a member they share no channel with → **403**; with someone they share a channel with → it rings. Inviting the first into a huddle the guest is in rings nobody.
- 🟢 **Bot:** B creates an incoming webhook for a channel B has on *All new messages*, then posts to it with B's app in the background → B's push reads "<bot name> in #channel", not B's name.
- 🟢 **Desktop switch:** *Preferences → Notifications → Desktop notifications while the app is open* → *Turn off*; with another app in front a mention plays a sound but shows no desktop notification. *Turn on* asks the browser when it has not been asked.

---

## 9. Realtime (presence, typing, live delivery)

> Best tested with **two browsers / two users**, and for scaling, **two realtime
> nodes** (run a second `chat-realtime` on another port behind the proxy).

### 9.1 🟢 Presence (online/offline)
- **Steps:** user A and B both online → A sees B as online → B closes the tab/logs out → A sees B go offline within ~60s (TTL) or immediately on clean disconnect.
- 🟠 **Edge — multi-tab:** B opens two tabs, closes one → B stays **online** (other tab/connection holds presence). Close the last → goes offline. (Presence is keyed per connection/node, so one tab closing doesn't flip you offline.)
- 🔴 **Edge — presence is workspace-scoped:** A and B share **no** workspace (C is in a workspace with B but A is not). A should **never** see B's presence — neither the initial `presence.batch` snapshot on subscribe nor live `presence.changed` updates reach A. Verify B going online/offline shows up for C (shared workspace) but not for A. (`presence.changed` carries the subject's `workspace_ids` and is delivered only to connections subscribed to a shared workspace.)
- 🟠 **Edge — a workspace joined after connecting:** A accepts an invite to a workspace B is in, without reloading, and switches to it → B sees A online at once.
- 🟢 **Away on purpose:** B picks *You → Set yourself away* → A sees B's dot turn amber at once, in every workspace they share. B types and clicks around → still away. B reloads, and opens the app on a phone → away on both. *Set yourself active* (or `/away` again) → A sees B online.
- 🟢 **Away when idle:** B leaves the app untouched (or hides the tab) for ten minutes → A sees B away within about thirty seconds after that. B moves the mouse → A sees B online within a second or two. B closes the last tab → offline.

### 9.2 🟢 Typing indicators
- **Steps:** A types in a channel → B (viewing it) sees "A is typing…" → stops → indicator clears.
- 🔴 **Edge:** typing should only reach members of that channel (not a non-member).

### 9.3 🟢 Live message delivery + ordering
- **Steps:** A sends several messages quickly → B sees them in the **correct order**, no gaps/dupes.
- 🟠 **Edge — reconnect:** kill the realtime connection (stop `chat-realtime` or toggle network) → the client shows "disconnected" and reconnects with backoff; after reconnect, new messages flow again.
- 🔴 **Edge — WS authz:** as user B, try to subscribe to a channel you're NOT a member of (craft a WS `channel.join` with another channel's id via devtools console) → you receive **no** messages for it. (Valid client message `type`s are `subscribe`, `channel.join`, `channel.leave`, `typing.start`, `typing.stop`, `ping`, plus the `huddle.*` set — anything else is ignored.)
- 🔴 **Edge — WS token type:** connect the websocket using a reset/refresh token in the `access_token` cookie → connection refused (only access tokens open a socket).
- 🟠 **Edge — waking up:** with the app open, turn Wi-Fi off for a minute (or sleep the laptop), then on → the *Offline* bar clears within a second or two, without waiting for the backoff; messages sent meanwhile by B are there. In devtools, *Offline* then back *Online* does the same.
- 🟠 **Edge — a gap too long to replay:** stop `realtime`, have B post more than 1 000 events' worth (or wait past the hour), start it again → the open channel, its unread counts, pins and the sidebar are all current without a reload.

### 9.4 🟠 Realtime health & consumer supervision
- **Reach:** `GET /readyz` on api (`:3000`) and realtime (`:3004`); `GET /metrics` on both.
- **Steps:** with the stack healthy, `curl localhost:3000/readyz` and `curl localhost:3004/readyz` → both **200 `ready`**.
- 🔴 **Edge — kill Redis flips realtime to 503:** `docker stop redis` → `curl -i localhost:3004/readyz` returns **503** (`redis unavailable`); api `/readyz` also 503s. Restart Redis → both go back to 200. (Realtime `/readyz` checks DB + Redis PING + that the supervised event consumer's heartbeat is <60s old; the consumer auto-restarts with backoff, and `realtime_consumer_heartbeat_age_seconds` in `/metrics` should stay small while events flow.)

### 9.5 🔴 WS upgrade hardening (Origin + revocation)
- 🔴 **Edge — Origin check:** open a WS to `/ws` from a page whose `Origin` is not in `CORS_ORIGINS` (e.g. via a script on another host) → upgrade **rejected** ("Origin not allowed"). Same-origin app connects fine.
- 🔴 **Edge — revoked session can't connect:** suspend a user (§2.2), then try to open a fresh `/ws` with their (still-unexpired) access token → upgrade **rejected** ("Session revoked").

### 9.6 🟠 A new version while a tab is open
- **Steps:** open the app, then rebuild and restart `frontend` (`docker compose --profile frontend up -d --build frontend`) → within ten minutes, or at once after switching to another tab and back, *A new version is available* shows at the top → *Reload* opens the new build and the bar is gone.
- 🟠 **Edge — a chunk that is gone:** with a tab from the old build still open, open something lazy you had not opened yet (*Preferences*, the emoji picker) → it loads, from the previous release's chunks or after one automatic reload; it never stays blank.
- 🟢 **Headers:** `curl -sI localhost:8080/ | grep -i cache-control` → `no-cache`; the same for `/version.json`; an `/assets/*.js` file is `public, immutable`.

---

## 10. Webhooks & reminders (admin integrations)

### 10.1 🟢 Outgoing webhook
- **Reach:** workspace Admin → Integrations/Hooks (or `POST /api/workspaces/<id>/hooks`).
- **Steps:** create an outgoing webhook pointing at a request-bin you control (e.g. `https://webhook.site/...`) → post a message in the workspace.
- **Expected:** the bin receives the event with an `X-ChatSystems-Signature: sha256=…` HMAC header; the execution is logged.
- 🔴 **Edge — SSRF:** create a webhook pointing at an internal address: `http://127.0.0.1:3000/...`, `http://169.254.169.254/latest/meta-data/` (cloud metadata), `http://10.0.0.5/`, or an IPv6 ULA → dispatch is **rejected/blocked**, the request is never made to the internal target.
- 🔴 **Edge — authz:** as a non-admin, try to create/list/read/delete hooks (`/api/workspaces/<id>/hooks`) → 403; secret fields are redacted (`***`) on read.
- 🔴 **Edge — demoted creator:** a second admin creates the webhook, then the owner makes them a *Member* → the next message sends nothing, and *Integrations* shows it *Disabled — its creator is no longer allowed to make it*.
- 🟢 **Turn on (CS-094):** on that disabled webhook the owner clicks *Turn on* → it is on again, the next message is delivered, and *Audit log* has *Integration turned back on*. The demoted member gets 403 from `PATCH /api/hooks/<id>`.

### 10.2 🟢 Reminders
- **Steps:** create a reminder for yourself → it fires around the due time (delivered as a notification).
- 🔴 **Edge:** try to create a reminder targeting another user as a non-admin → 403.

### 10.3 🟢 Incoming webhook (token-authed, Slack-compatible)
- **Reach:** workspace Admin → create a hook (`POST /api/workspaces/<id>/hooks`) with `hook_type: "incoming_webhook"` and `config: { "channel_id": "<channel in this workspace>" }`. The server **mints `config.token`** and binds the hook to that channel; the creator becomes the message author.
  ```bash
  # create (admin session/bearer required); copy config.token from the response
  curl -s -X POST localhost:3000/api/workspaces/<WS_ID>/hooks -H "Authorization: Bearer $TOKEN" \
    -H 'Content-Type: application/json' \
    -d '{"hook_type":"incoming_webhook","name":"ci","config":{"channel_id":"<CH_ID>"}}' | jq
  # post a message — NO auth header, just the token in the URL (Slack-style {"text":...})
  curl -s -X POST localhost:3000/api/hooks/incoming/<TOKEN> -H 'Content-Type: application/json' \
    -d '{"text":"deploy finished ✅"}' | jq
  ```
- **Expected:** the unauthenticated POST returns `{"status":"ok","message_id":…}` and the message **appears live in the bound channel** (authored by the hook creator).
- 🔴 **Edge — bad token:** POST to `/api/hooks/incoming/<garbage>` → **401** "Invalid webhook token" (and a suspended/inactive hook is rejected too).
- 🔴 **Edge — config requires channel:** creating an `incoming_webhook` without `config.channel_id` (or with a channel from another workspace) → **400/validation error**, no hook created.
- 🟠 **Edge — rate limit:** hammer the same token >60×/min → **429** (per-token limit), the channel isn't flooded.

### 10.4 🟢 A channel's integrations
- **Reach:** a channel's ⋯ → *Channel details → Integrations*.
- **Steps:** as Admin, make an incoming webhook for #random in *Workspace settings → Integrations*, an outgoing webhook that forwards #random, and a workflow whose trigger is a reaction in #random. Make Bob a channel admin of #random. As Bob open *Channel details → Integrations* in #random, *Add an incoming webhook* named *Bob's CI*, copy the URL and `curl` `{"text":"from Bob"}` to it. Make a new URL for it, then remove it. As Charlie (plain member) open the same tab.
- **Expected:** Bob and Charlie both see the incoming webhook (with who added it), the outgoing webhook, the workflow (On/Off, who built it) and any command scoped to #random, never a URL or secret. Bob's URL shows once, under his webhook; the `curl` lands in #random; the old URL stops working after the new one is made. Charlie has no add, rotate or remove. A workflow row has *Open* for someone who may build workflows, which opens it in the *Workflows* panel. Each of Bob's actions is a row in *Audit log*.
- 🔴 **Edge — authz:** as Bob, `POST /api/workspaces/<id>/hooks` an incoming webhook for a channel he does not run, an outgoing webhook or a slash command → **403**.

### 10.5 🟢 A slash command that answers later
- **Reach:** workspace Admin → *Integrations* → a slash command `/deploy` whose URL is a request-bin that answers `200` with an empty body (webhook.site: *Edit → Content* empty).
- **Steps:** in #general type `/deploy staging` → copy `response_url` from the bin's request, then:
  ```bash
  curl -s -X POST '<RESPONSE_URL>' -H 'Content-Type: application/json' -d '{"text":"only for you"}'
  curl -s -X POST '<RESPONSE_URL>' -H 'Content-Type: application/json' -d '{"text":"deploying…","response_type":"in_channel"}'
  curl -s -X POST '<RESPONSE_URL>' -H 'Content-Type: application/json' -d '{"text":"deployed ✅","response_type":"in_channel","replace_original":true}'
  ```
- **Expected:** the composer shows */deploy is working on it…* (pulsing) at once; the first `curl` replaces it with *only for you*, which a second person in #general never sees; the second posts *deploying…* as the bot, and the third edits that message to *deployed ✅* instead of posting another. The bin's request has `X-Chat-Request-Id` equal to the body's `request_id`.
- 🔴 **Edge — used up:** two more `curl`s work, the sixth → **404** *This response URL has expired or been used up*; changing one character of the token → **404** *Unknown response URL*.
- 🟠 **Edge — app down:** point the command at `https://httpbin.org/status/503` → the composer shows */deploy is taking a moment…*, and about ten seconds later */deploy could not be reached*. Point it at `https://httpbin.org/status/500` → an error at once, one attempt.

### 10.6 🟢 A Slack import that looks like Slack did
- **Reach:** *Workspace settings → Slack import*.
- **Steps:** import an export whose `users.json` has a guest (`is_restricted`), an account with `deleted: true`, an archived channel in `channels.json`, and reactions such as `:face_with_rolling_eyes:`, `:+1::skin-tone-4:` and a custom `:shipit:` (with `SLACK_TOKEN` set or an `emoji.json` in the zip). Then start another import, restart the worker (`docker compose restart worker`) while it says *running*, and wait five minutes.
- **Expected:** the counts show guests, *who left Slack* and *archived*. The guest is a guest under *Members* and sees only its channels; the deleted person's messages carry their name and the account is suspended in the instance admin page; the archived channel shows its history with the archived bar; the reactions are 🙄, 👍🏽 and the custom image, and an unknown one is listed under *not imported*. The interrupted run turns *failed* with *The import stopped when the worker did*; *Start again* queues it and it completes without duplicating messages.
- **Edge:** with a local private `#incident-2023` and a public `#general-chat`, import an export where both exist (the Slack `#general-chat` archived), then import it again → Slack's private history lands in `#incident-2023-slack` and nobody joins the local one; `#general-chat` stays open after both runs. Reactivate the suspended leaver in the instance admin page and invite them → they finish the sign-up with a password.

---

## 11. Cross-cutting security & resilience (mostly API-level)

| # | Case | How to reach | Expected |
|---|---|---|---|
| 11.1 🔴 | Cross-tenant read | log in as B, hit `GET /api/workspaces/<A's id>/...` (members, channels, hooks, files) | **403** everywhere |
| 11.2 🔴 | Login brute-force (per-email) | POST `/api/auth/login` wrong pw >10× for one email within 15 min | **429 Too Many Requests** |
| 11.2b 🔴 | Login brute-force (per-IP) | POST `/api/auth/login` >30× within 15 min from one IP, varying the email each time | **429** (secondary per-IP limit, keyed on `X-Forwarded-For`/`X-Real-IP`, so spraying many accounts from one host is also throttled) |
| 11.3 🟠 | Redis down (fail-open) | stop the `redis` container, then log in with correct creds | login still **succeeds** (rate-limit fails open, doesn't lock everyone out) |
| 11.4 🔴 | Error leakage | trigger a 500 (e.g. stop Postgres mid-request) | response body is generic `{"error":"internal server error"}` — **no SQL/internal detail**; detail only in server logs |
| 11.5 🟠 | Graceful shutdown | `docker stop` the api with an in-flight request | request drains (not abruptly cut); api `/readyz` flips to 503 when DB/Redis down. Realtime `/readyz` additionally 503s if the event consumer stalls (>60s since last heartbeat) — see §9.4. |
| 11.6 🟠 | Body limit | POST a >100 MB JSON/file | rejected, not OOM |
| 11.7 🔴 | Missing/invalid token | call any `/api/...` (non-auth) without a token, or with a garbage token | **401**, flat "invalid or expired token" (no library internals) |
| 11.8 🔴 | SSO after suspension | sign in once with *Continue with SSO*, suspend the account in *Instance admin*, sign in with SSO again | refused, "This account is suspended"; no session |
| 11.9 🔴 | Hard erase | as Ana, edit a message Bob then answers, react to Bob's, attach a file; *Instance admin* → erase Ana with *hard delete* | her answered message is an empty tombstone with no edit history, her reaction and file are gone, and the file's download link answers 404 |

---

## 12. Known limitations to verify *don't* regress (not bugs)
- Presence is eventually-consistent across nodes (≤60s TTL on hard node crash) — a clean disconnect is immediate.
- Webhook dispatch validates DNS at request time (DNS-rebinding is out of scope); redirects are disabled.
- File search reads text inside PDF, Office and text files, not text in images (no OCR), and not in a file over 25 MB or a password-protected PDF.
- A huddle holds at most eight people until the SFU (CS-037): a ninth person who presses *Join* gets "This huddle is full" and no call starts; the eight already in it see nobody join or leave.

---

### Quick triage if something fails
1. `docker compose ps` — all healthy? `/readyz` green?
2. `docker compose logs api realtime` — JSON logs; find the `request_id` from the response and grep it.
3. Postgres reachable on 5433 (dev) / inside-network 5432; Redis on 6380 (dev) / 6379.

## 12. On a phone (what emulation cannot check)

The Playwright `mobile` project covers layout at a phone width. These are the
things a headless Pixel cannot tell you, and each one has bitten this app at
least once.

Install the PWA on a real device first: iOS *Share → Add to Home Screen*,
Android *menu → Install app*. A secure context is required, so this needs the
HTTPS deployment rather than `http://192.168.x.x`.

- [ ] **Keyboard.** Tap the composer. The page must not zoom (that is the 16px
      rule), the composer must stay visible above the keyboard, and the message
      list must still be scrolled to where it was when the keyboard closes.
- [ ] **Safe areas.** On a notched or gesture-bar device, the send button is not
      under the home indicator, and the channel header is not under the notch —
      in the installed PWA as well as in the browser, which crop differently.
- [ ] **Drawer.** Open the navigation drawer, then open a dialog from it
      (Browse channels). The dialog must cover the screen, not sit inside the
      drawer. A transformed ancestor is the containing block for
      `position: fixed`, which is what made this wrong before the portal.
- [ ] **Uploads.** In the installed app, the composer's paperclip, *Edit profile →
      Upload photo* and *Workspace settings → General → Upload icon* each open the system picker
      (photos, camera, files), and the chosen file uploads. Emulation cannot check this:
      the picker that failed here was the device's own.
- [ ] **Back gesture.** From a thread, the system back gesture returns to the
      channel rather than leaving the app.
- [ ] **Background and foreground.** Lock the phone for a minute, come back:
      the socket reconnects and missed messages backfill.
- [ ] **Push.** With the app fully closed, a mention arrives as a system
      notification, and tapping it opens that message. On iOS this only works
      for the installed PWA.
- [ ] **Huddle.** Joining is audio-only and says so; the call survives the screen
      locking.

---

## 13. Dialogs, in light, dark and at a phone width

Every dialog shares one frame, so a fault in one usually shows in all of them.
Open each below three times: light theme, dark theme, and a phone width (the
dialog becomes a sheet from the bottom).

- **Dialogs:** Create channel, Browse channels, Channel details (and its
  archive confirmation), New message, Forward, Keyboard shortcuts (`?`), Create
  workspace, Delete workspace, Edit profile, Preferences, Incoming call, and the confirmations
  for deleting a message and an integration.
- [ ] **Width.** Each is one of three widths (400, 520, 720 px); opening one
      after another, the frame does not jump between two sizes of the same kind.
- [ ] **Scroll.** In a long one (Edit profile at a phone width, Browse channels with many channels,
      Keyboard shortcuts) only the middle scrolls. The title and the buttons stay
      put, and a hairline appears above or below the middle only while there is
      more to scroll to.
- [ ] **Fields.** Every field has a label, the same 36 px height and the same
      text size; checkboxes and selects look like the app, not the operating
      system, in both themes.
- [ ] **Errors.** Submitting Create channel with a name that exists marks the
      field red and says why under it; Channel details does the same.
- [ ] **Closing.** The X, Cancel, Escape and a click on the backdrop each close
      it with a short fade; with *Reduce motion* on in the OS, it closes at once.
- [ ] **Focus.** Opening puts the cursor in the first field (or on the action of
      a confirmation); Tab stays inside; closing returns focus to what opened it.
- [ ] **Create channel.** Typing `Plan Budget` shows `plan-budget`; the counter
      counts down from 80; the Public / Private cards switch with a click and
      with the arrow keys.
- [ ] **Browse channels.** The count follows the search; the sort reorders by
      name, by members and by age; *Create channel* in the header opens the
      create dialog; Join or Leave appears on the row under the pointer and on
      the row that has keyboard focus, and on a phone on every row.
- [ ] **Confirmations.** Deleting a message shows that message; archiving a
      channel names it; deleting an integration names it. Enter confirms, Escape
      cancels, and a failure keeps the dialog open with the reason.
- [ ] **Profile and Preferences.** *Edit profile* saves the name, status and bio with one
      button and closes; a status left untouched keeps its *Clears at*. *Preferences* shows one
      section at a time, ↑/↓ move between them, and the theme changes the moment it is picked.
- [ ] **Two workspaces.** In one, *Edit profile* sets another name and a status, and the
      *You* menu pauses notifications; switch to the other: the account's name, no status,
      notifications on. *Preferences → Account* changes the name both show where neither has
      its own; *Preferences → Notifications* names the workspace it changes.
- [ ] **An account photo everywhere.** *Preferences → Account → Upload photo* shows *Photo
      updated.* at once; switch workspace and the photo is there too, and someone who shares
      only the other workspace sees it on your messages. Set a file retention of one day in the
      workspace you uploaded from and run the worker: the photo stays.
- [ ] **A workspace photo stays (CS-094).** *Edit profile → Upload photo* in a workspace with a
      file retention of one day, save, run the worker: the photo stays, a member sees it, and
      somebody who shares only another workspace with you gets 403 on its URL.
