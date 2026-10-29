# Changelog

What changed between releases, for the people who run an instance. The reasoning behind each
change is in [docs/history.md](docs/history.md); what an upgrade needs from you is in the
upgrade notes of [docs/RUNBOOK.md](docs/RUNBOOK.md).

Images are published for every release:

```
ghcr.io/petar-basic/chat-systems-backend:<version>
ghcr.io/petar-basic/chat-systems-frontend:<version>
```

Set `BACKEND_IMAGE`, `FRONTEND_IMAGE` and `VERSION` in `.env` to run them instead of building
from source (see [Upgrade](docs/RUNBOOK.md#upgrade)).

## [Unreleased]

### Upgrading from 3.7.0

- **Set `SECRETS_KEYS` before you upgrade**: `echo "k1:$(openssl rand -base64 32)"`, on the
  `api` and the `worker`. Both refuse to start without it. Keep `JWT_SECRET` unchanged for
  this start: the `api` re-seals what 3.7.0 sealed with it (see the RUNBOOK upgrade note).
- **Set `JWT_SIGNING_KEYS` and `JWT_PUBLIC_KEYS` before you upgrade** (the commands are in
  `.env.example` and the RUNBOOK): the private keys on the `api` and the `worker`, the public
  halves on `realtime`. Upgrade all three together. **Everybody is signed out once**, because
  the HS256 sessions 3.7.0 issued are refused. Remove `JWT_SECRET` once the `api` has logged
  that every stored secret is on a `SECRETS_KEYS` key.
- Migration 82 adds `users.manual_away`; migration 83 adds a key id beside every sealed
  secret; migration 84 adds `command_invocations`.
- For WebSocket clients: `presence.batch` and `presence.changed` carry `away` as well as
  `online` and `offline`, and `presence.changed` now also reaches the person it is about.
  For API clients: `PUT /users/me/presence` with `{ "away": true }` sets yourself away.
- For WebSocket clients: a `command.response` frame carries a slash command's late answer to
  whoever ran it. For slash-command apps: the payload has `request_id` and `response_url`,
  the request an `X-Chat-Request-Id` header, and an empty 2xx body now means *answering
  later* instead of an error.

### Added

- **Away.** *You → Set yourself away* (or `/away`) shows you away to everyone, in every
  workspace and on every device, until you set yourself active. Ten minutes without using the
  app shows you away too, and the next keystroke shows you active.
- **Slash commands that take their time.** A command's app may answer at once, or acknowledge
  and answer up to five times in the next 30 minutes through a `response_url`, in the channel
  or only to whoever ran it. An app that does not answer in three seconds is tried twice more
  with the same request id, and the person sees *taking a moment…* instead of an error.

### Security

- **Stored secrets have keys of their own.** Connected apps' client secrets and tokens, TOTP
  secrets and the setup token are sealed with `SECRETS_KEYS` instead of `JWT_SECRET`.
  Rotating `JWT_SECRET` no longer breaks every connected app and second factor, and
  `SECRETS_KEYS` rotates on its own: add a key, put it first, restart, and the API re-seals
  everything, so nobody sets an app up again or reconnects.
- **Sessions are signed with Ed25519, and the realtime gateway can no longer sign one.**
  Tokens are EdDSA with a `kid`; the API and the worker hold the private keys and the gateway
  only the public halves, so a leaked gateway configuration mints nothing. HS256, unsigned
  tokens and unknown key ids are refused. Signing keys rotate without signing anybody out:
  add a key, promote it, retire the old one after the refresh-token lifetime.

## [3.7.0] — 2026-10-29

### Upgrading from 3.6.0

- Pull both images and restart `api`, `realtime`, `worker` and `frontend` together; the API
  runs migrations 80–81 on start, which add `slack_channels.created_by_import` and lowercase
  every email address, so snapshot first. On its first start the worker moves workspace photos
  into the object store's `avatars/` prefix and workspace icons into `icons/`, outside file
  retention (see the RUNBOOK upgrade notes); going back to 3.6.0 means restoring the snapshot.
- **Check for addresses that differ only in case.** Migration 81 refuses mixed-case addresses
  from then on. If two accounts differ only in case it stops before changing anything and names
  them, and the `api` does not start until one is erased or given an address of its own; the
  RUNBOOK says how.
- Channels mapped by an earlier Slack import count as ones it merged into, so a re-run never
  archives them.
- Reactivating an account that has no password and no SSO link (an imported Slack leaver), from
  the admin page or with SCIM `active: true`, leaves it `pending` instead of `active`; invite
  the person to let them in.
- For API clients: `DELETE /admin/users/{id}/data` reports `removed.edits`, `reactions`,
  `votes`, `comments`, `scheduled` and `files` beside `messages`. Taking over a workflow
  gives it a new `webhook_url`, and `POST /workflows/{id}/rotate` makes one on demand; the
  old URL answers 401 in both cases.
- For API clients: `POST /conversations/{id}/read` is gone; a conversation is read like a
  channel, `POST /channels/{id}/read` with the message read up to. Unread counts carry
  `last_read_at` and a first message page `last_read_before_at`, the time of the read.
- For API clients: a workspace photo is uploaded with `POST /workspaces/{id}/profile/avatar`
  and a workspace icon with `POST /workspaces/{id}/icon`; either uploaded through
  `/files/upload` is an ordinary file and no longer readable by the workspace.
  `PATCH /hooks/{id}` with `{ "is_active": true }` turns a stopped hook back on.
- For API clients: `POST /workspaces/{id}/invites` answers `email_status` (`sent`,
  `not_configured`, `failed`, or `null` for a link invite) instead of `email_sent`. Search hits
  from `/search` and `/search/files` carry `channel_name`. A reply to a deleted root with no
  replies left answers 404 instead of 422.

### Security

- **SSO lets nobody around a suspension.** An account already linked to the provider was
  signed in without its status being checked, so a suspended or erased person got a session
  through SSO. Erasing an account now also removes its SSO link.
- **A hard erase removes everything they wrote.** A message others answered kept every earlier
  version in its edit history, and their files, reactions, votes, list comments, scheduled
  messages and the notifications quoting them stayed. All of it goes now, files with their
  stored objects.
- **A hook stops when its creator loses the role.** An outgoing webhook or slash command made
  by an admin who is now a member kept sending channel traffic; it turns off, saying why.
- **A taken-over workflow gets a new URL.** The old owner could still start it from the link
  they knew. *New URL* in the workflow replaces it at any time.
- **A Slack import keeps out of private channels.** A private Slack channel merged into a local
  private channel of the same name, adding Slack's members to it and its history to theirs.
  Private channels never merge now; a taken name becomes `<name>-slack`.
- **A re-import leaves a merged channel open.** Running an import again, or *Start again*,
  archived a local channel the first run had merged an archived Slack channel into.
- **An archived channel is read-only all the way.** Editing, deleting and pinning a message,
  voting in a poll, adding or removing members, bookmarks, renaming and starting a huddle still
  changed an archived channel; every one answers 403 now, and the client hides them.
- **A guest calls only whom they could message.** Starting a DM huddle checked only that the
  other person was in the workspace, so a guest could ring a member they could not message, and
  invite them into a huddle. Both follow the DM rule now.

### Fixed

- **An archived private channel can be found again.** The sidebar leaves archived channels out
  and *Browse channels* listed public ones only, so nobody could reach one to unarchive it. Browse
  now lists those you are a member of, under the archived public ones.
- **A message that arrives while a channel opens is shown.** The first page of a channel was
  still loading when a new message came in over the socket, so the message was dropped until
  the next reload. It now reloads the page instead.
- **Read state matches what you read.** *Mark all read* could leave a channel's read position
  on a reply kept in its thread, so opening it loaded the whole history looking for it. A
  mention in a reply also sent to the channel stayed unread after reading the channel, and its
  email went out. A reply notification under a deleted message opened nothing, a DM opened at
  an old *New* line, and a keyword quoted in a forward alerted its owner. Each now behaves as
  it does in a channel.
- **A failed send stays where you sent it.** A reply from the *Threads* view that failed
  vanished from its card, a failed message was lost on reload, a send the server took just before
  the request failed showed twice, and scheduling dropped a file that had failed to upload. The
  card shows the failed reply with *Retry* and *Discard*, failed messages survive a reload, a
  delivered message never keeps a failed copy, and scheduling refuses a failed file as sending
  does.
- **An address works in any case.** An invite to `Ana@Firma.com` made an account SSO could not
  find as `ana@firma.com`, so it refused or made a second one, and signing in with the password
  needed the exact case. Addresses are stored lowercase now.
- **A workspace photo stays.** It was a file of that workspace, so file retention deleted it with
  old attachments. It has a place of its own now, shown to that workspace's members.
- **A stopped hook can be turned back on.** One switched off because its creator left or lost
  the channel could only be deleted and made again; *Integrations* has *Turn on*, and whoever
  turns it on becomes its creator.
- **Push goes to the device you are using.** A laptop shut mid-use counted as active for up to
  two and a half minutes, holding back push to the phone; it now stops within about 30 seconds,
  and leaving or hiding the page says so at once. The push badge counted notifications from
  workspaces you had left, and a desktop notification from a second server opened nothing. The
  badge now matches the app's, and the click opens the message on its server.
- **The survey's small things.** A search hit from a public channel you are not in names its
  channel. A thread whose deleted root lost its last reply shows no composer. An export's date
  range covers edits, reactions, polls and votes, not only messages and files. An invite whose
  email failed says so instead of claiming email is not set up. A card in *Threads* has
  *Also send to* and scheduling, as the thread panel does.
- **Everyone sees the workspace icon, and it stays.** It was an ordinary file with no message,
  so only the admin who uploaded it could load it, file retention could delete it, and a
  replaced one was never removed. It has a place of its own now, shown to the workspace's
  members.

## [3.6.0] — 2026-10-28

### Upgrading from 3.5.0

- Pull both images and restart `api`, `realtime`, `worker` and `frontend` together; the API
  runs migrations 75–79 on start, which add the `huddle_invites` table,
  `hooks.disabled_reason`, a thread target on scheduled messages, one pending mention email
  per person and message, and `slack_imports.heartbeat_at`, so snapshot first. On its first
  start the worker moves every account photo into the object store's `avatars/` prefix (see
  the RUNBOOK upgrade note); 3.5.0 cannot read photos there, so going back means restoring
  the snapshot.
- **Re-send pending email invites.** An emailed invite link now names the invite it stands
  for, and links sent before the upgrade do not: they answer *Invalid or expired invite*.
  Revoke and re-send any email invite that has not been accepted yet. Shared invite links are
  unaffected.
- For WebSocket clients: `huddle.join` needs only `huddle_id`; `channel_id`, `workspace_id`
  and `dm_partner_id` are ignored.
- For API clients: `GET /messages/{id}/thread` takes `limit` and `cursor` instead of
  `offset`, returns the newest replies first, and includes reactions. A deleted message with
  replies is listed as a tombstone (`deleted_at` set, empty `content`).
- For API clients: `copy_attachments_from` on `POST /channels/{id}/messages` is now
  `forwarded_from`; it also records `metadata.forwarded_from`.
- For API clients: reading a channel no longer marks its threads' notifications read;
  `POST /threads/{id}/read` does. A push's `badge_count` is the total across workspaces, and a
  thread reply not sent to the channel no longer counts in the channel's unread counters.
- For WebSocket clients: send `{ "type": "activity", "active": bool }` when the person starts
  or stops using the app. Push is held back only while a session says it is active, so a
  client that never sends it gets push even with its socket open. A `huddle.ring` to somebody
  who paused the workspace is no longer delivered.
- For API clients: `POST /users/me/avatar` takes a multipart `file` and sets the account photo.
  An account photo set with `PATCH /users/me` to a workspace file is no longer readable to the
  workspace.
- For API clients: `ImportRun` carries `retryable`, and
  `POST /workspaces/{ws_id}/slack-imports/{import_id}/retry` queues a failed run again. An
  import report counts `guests`, `users_suspended` and `channels_archived`.
- Building the frontend image yourself: pass `--build-arg PREVIOUS_IMAGE=<the frontend image
  you run now>` to keep its chunks for tabs opened before the upgrade, and
  `--build-arg APP_VERSION=<version>` to name the build. A proxy in front of the frontend
  should not cache `index.html` or `version.json`; nginx now sends `no-cache` for both.
- For API clients: posting, reacting, scheduling and incoming webhooks answer 403 on an
  archived channel. `POST /channels/{id}/unarchive` undoes `DELETE /channels/{id}`, both
  publish `channel.updated` (`is_archived`), and *Browse channels* lists archived channels
  with `is_archived`.
- For API clients: `POST /workspaces/{id}/invites` returns the invite with `invite_url` and
  `email_sent`, and `GET /workspaces/{id}/invites/{invite_id}/link` hands out a pending email
  invite's link again. An empty `SMTP_HOST` now means no email: invites say so instead of
  claiming they were sent.

### Security

- **A private channel's huddle is its members'.** Joining a huddle is checked against the
  huddle itself: its channel's members, the two people of a DM huddle, or someone invited
  from inside it. *Active huddles* lists only channels you are in, and only someone in a
  huddle can invite to it.
- **An email invite ends when the invite does.** Revoking it, its expiry and its use limit now
  stop the emailed link too, and the role it grants is the one the invite has today.
- **A private channel's member list is its members'.** `GET /channels/{id}/members` follows
  the same access as reading the channel.
- **A suspended or removed person stops acting.** Suspending or erasing an account turns off
  every webhook, slash command and workflow that person set up, and removing someone from a
  workspace turns off theirs there; activating the account again does not turn them back on.
  Each scheduled message, workflow run and webhook also checks its author when it fires: a
  suspended author's scheduled message fails with `author_inactive`, and a webhook whose
  creator can no longer read its channel turns itself off and says why under *Integrations*.
- **A workflow whose builder left can be taken over.** A workspace admin opens it under
  *Workflows* and chooses *Take over*; it becomes theirs, stays off until they turn it on, and
  the audit log records it.
- **A private Slack channel stays private.** The Slack import merges a channel into an
  existing one of the same name only when both are the same kind; otherwise it imports it as
  `<name>-slack` and says so in the report.

### Fixed

- **Opening a message lands on it.** From *Activity*, a desktop notification, a phone push or
  search, a direct message opens its conversation, a thread reply opens its thread scrolled to
  it, and a public channel you have not joined opens on the message with *Join channel*. Old
  message links resolve the same way, and a message that is gone says so instead of opening
  `#general`. `GET /messages/{id}/location` says where a message lives; push payloads and the
  `notification` frame carry `thread_parent_id`.
- **Search goes past twenty.** *Show more* pages through message and file results.
- **Threads keep up.** A long thread reads back to its first reply with *Show earlier
  replies*. Deleting a reply takes it off the count, and an open thread shows other people's
  reactions, pins and deletions as they happen. A deleted parent with replies stays as *This
  message was deleted*, so its thread stays together; retention removes a thread once its last
  reply is past the period, and no reply ever appears in the channel on its own. Replies can
  be sent to the channel too with *Also send to #channel*, and scheduled from the thread
  composer.
- **Sending never loses work.** A message or thread reply that fails to send keeps its text
  and files, survives a refresh of the list, and offers *Retry* (with the same files) or
  *Discard* (back into the composer). Files in the composer stay with their channel when you
  switch away, and a file that failed to upload says why and must be retried or removed
  before sending. A scheduled message that failed shows the reason with *Send now* and
  *Remove*. Forwarding a message no longer notifies the people or the `@channel` in the
  quote.
- **Unread, Activity and the badge agree.** Reading a channel or DM clears its Activity rows,
  reading a thread clears the thread's, and a message or mention that arrives in the channel
  you are looking at is read at once instead of leaving a badge. A reply kept in its thread no
  longer marks the channel unread. A mention from another workspace no longer lands in the
  open one's sidebar, and the app icon always shows the total across workspaces, push
  included. A channel with unread messages opens on the first of them under the *New* line,
  also on a fresh load, with *Jump to first unread* while it is above the view; *New
  messages* sits over the list again instead of under it.
- **Notifications arrive once, where they should.** *Email me what I missed* goes out for a
  mention or DM still unread five minutes later, whether or not a push was sent, never for one
  you have read, once however often the event is redelivered, and with a link to the message
  (or the workspace, for several). An open tab nobody is using no longer holds back push:
  push waits only while a window is in front and in use. A paused workspace does not ring for
  huddles, and the pause menu names the workspace it pauses. A webhook's creator is notified
  of what their hook posts, under the bot's name. *Preferences → Notifications* has the
  *Desktop notifications while the app is open* switch, and a desktop notification and a push
  about the same channel replace each other instead of showing twice.
- **Waking up reconnects, and a deploy no longer breaks an open tab.** The socket reconnects
  as soon as the network comes back or the tab is shown again, instead of waiting out its
  backoff, and a socket left half-open by sleep is replaced within two seconds. Switching to
  a workspace you joined after connecting shows you online there at once. A gap too long to
  replay now also refreshes unread counts, pins and the sidebar. After a deploy, an open tab
  shows *A new version is available* with *Reload*, a lazy page or the emoji picker whose
  chunk is gone reloads once onto the new build, and release images keep the previous
  release's chunks.
- **An archived channel is closed.** Nothing can be posted, reacted, scheduled or sent by a
  webhook into it; its composer becomes an *Archived* bar, and workspace admins and the
  channel's admins can *Unarchive* it there. Archiving and unarchiving post a line in the
  channel and reach everyone viewing it at once.
- **An export without DMs has no DM files.** `files.jsonl` follows the same DM choice and date
  range as the messages.
- **A retention audit row counts its own workspace.** Each workspace's *Retention purge* entry
  used to carry the totals of every workspace purged before it in the same run.
- **Joining a channel you were looking at goes live.** After *Join channel* on a channel opened
  without being in it, new messages and changes arrived only after a reconnect.
- **Inviting works without email.** Every invite shows its link to copy, and the form says
  when email is not set up instead of *Invite sent*. *Workspace settings → Members* lists
  pending invites with *Copy invite link* and *Revoke*.
- **SSO from an invite lands in the workspace.** The invite page offers *Continue with SSO*,
  and the invite is used on the way back from the provider, activating the invited account.
- **Forgot password does the right thing.** Somebody invited but not registered gets their
  invite again, an SSO account is told to sign in with SSO, and a too-short new password no
  longer uses up the reset link.
- **An account photo shows everywhere and stays.** A photo set under *Preferences → Account*
  is the account's, not a file of the workspace that was open: every workspace you share with
  someone shows it, and file retention no longer deletes it. Replacing or removing a photo
  deletes the old one.
- **A status picked as *Edit profile* opens is kept.** The form showed before the workspace
  profile had arrived, and its arrival then reset the emoji already picked.
- **A Slack import keeps what it found.** Slack guests arrive as guests, people who had left
  Slack arrive as suspended accounts with their name still on their messages, and archived
  channels arrive archived. Reactions map through the full emoji set, skin tones included, and
  the workspace's custom emoji. An import that was running when the worker restarted is marked
  failed after five minutes, and *Start again* picks it up where it stopped.

## [3.5.0] — 2026-10-27

### Upgrading from 3.4.2

- Pull both images and restart `api`, `worker` and `frontend` together; the API runs
  migrations 73–74 on start, so snapshot first. Migration 73 copies each live status into every workspace
  the person is in (a custom emoji only where that workspace has it), then drops the status
  columns from `users` and the unused `user_preferences` and `bookmarked_messages` tables.
- Migration 74 does the same for notifications: a running pause, the notification schedule
  and the mention-email switch are copied into every workspace the person is in, and leave
  `users`. Nobody loses a pause or a schedule. Both drop columns, so going back to 3.4.2
  means restoring the snapshot.
- For API clients: `PUT`/`DELETE /users/me/status` are now
  `PUT`/`DELETE /workspaces/{id}/profile/status`, and `UserPublic` no longer has
  `status_emoji`, `status_text` or `status_expires_at`. Members gain `bio`.
  `/notifications/dnd`, `/users/me/notification-schedule` and `/notifications/email` are now
  `/workspaces/{id}/notifications/dnd`, `…/schedule` and `…/email`.

### Added

- **A profile of your own in each workspace.** Name, photo, bio and status are set per
  workspace in *Edit profile*; a status set in one workspace no longer shows in the others. A
  name, photo or bio left empty shows the account's, edited in *Preferences → Account*. The
  hover card shows the bio. API: `GET`/`PATCH /workspaces/{id}/profile`.
- **Notification settings of your own in each workspace.** Pausing notifications (the *You*
  menu, the Activity panel or `/dnd`), the notification schedule and *Email me what I missed*
  apply to the workspace they were set in; the others keep notifying. *Preferences →
  Notifications* names the workspace it changes.

## [3.4.2] — 2026-10-27

### Upgrading from 3.4.1

- Pull both images and restart `api`, `worker` and `frontend` so all three run 3.4.2; there is no migration.

### Fixed

- **React with a custom emoji.** The emoji picker has a *Workspace* category with the
  workspace's own emoji, in the composer, on a message and for a workflow's reaction trigger.
  A reaction used to be refused above eight characters, so `:party-parrot:` could not be one.
  API: `POST /messages/{id}/reactions` takes a `:shortcode:` the workspace has, and answers 422
  for one it does not.
- **The status emoji comes from the picker**, workspace emoji included, instead of a text box
  that took any sixteen characters. A custom one shows as its image wherever the status does;
  in a workspace without it, as its `:shortcode:`.

## [3.4.1] — 2026-10-27

### Upgrading from 3.4.0

- Pull both images and restart `api`, `worker` and `frontend`; there is no migration.

### Fixed

- **Custom emoji show.** Their image answered 404, because the download route only knew
  uploaded files, so a custom emoji was a broken image everywhere: in *Custom emoji*, in
  messages and in reactions. Anyone in the workspace can now load it, and a new one is stored
  under its image type, so local storage serves it as an image.
- **Typing `:name` offers the workspace's own emoji**, first, above the standard ones;
  choosing one inserts `:name:`.

## [3.4.0] — 2026-10-27

### Upgrading from 3.3.0

- Pull both images and restart `api`, `worker`, `realtime` and `frontend` together; the API
  runs migrations 71–72 on start, so snapshot first. Neither rewrites existing rows.
- For API clients, two shapes change in a way a strict client notices:
  `POST /workflows/{id}/publish` is now `POST /workflows/{id}/enable`, and
  `PATCH /workspaces/{id}/retention` replaces the whole policy, so a client that saves it must
  now send `channel_overrides` too, or it switches them off. Everything else is added.
- Migration 71 adds `retention_policies.channel_overrides` and the `channel_retention` table;
  channel periods are off for every workspace until an owner turns them on.
- Migration 72 (`instance_setup`) needs nothing on an instance that already has an instance
  admin, and it prints no setup token. `ADMIN_PASSWORD` is optional now; once the instance is
  up, remove it from `.env`.
- `POST /workspaces/{id}/workflows` creates a workflow that is already on. A workflow saved as
  a draft before the upgrade is one that is off; *Turn on* starts it.
- `GET /instance/info` gains `setup_required`. `GET /channels/{id}/messages` answers
  `{ data, unread_notifications? }`; the second field is there only when the request asked for
  `mark_read=true`, so a client that reads `data` sees no change.

### Added

- **A new workspace says what to do next.** While it has fewer than three members or no
  channel but `#general`, its admins see a checklist above the messages: *Invite your team*,
  *Create a channel*, *Set an icon*, *Connect SSO*. Each is ticked once done, and the list can
  be dismissed. An empty channel says what it is for and offers *Add people* and *Add a
  bookmark*.
- **First run from the browser.** With no instance admin, the API logs a one-time setup
  token on every start, and `/setup` takes it with the first admin's name, email and password
  and the first workspace's name. `ADMIN_EMAIL`/`ADMIN_PASSWORD` still work for scripted
  installs. API: `POST /setup`.
- **Send a test email**, at the end of setup and in *Instance admin → Email*, says in words
  why mail did not go: nothing listening at `SMTP_HOST`, the wrong TLS mode, or credentials
  refused. API: `POST /admin/email/test`.
- **Retention per channel.** An owner can let channels keep messages for their own period
  (*Data retention → Let channels keep messages for their own period*); a channel's admins then
  choose a number of days, forever, or the workspace default in *Channel settings*. The
  channel gets a message saying who changed it, and the audit log a `retention.changed` row.
  Files and lists keep the workspace period. API: `GET`/`PUT`/`DELETE
  /channels/{id}/retention`, and `channel_overrides` on the workspace policy.
- **Screens for what was API only.** *Workspace settings* gains *Data retention* (owner): keep
  messages and files forever or for a number of days, with the count the change would remove
  now shown before saving; *Exports*: start a workspace export, follow it, download it once;
  and *SCIM provisioning* (instance admins): create, rotate and revoke tokens, each shown once.
  An instance admin exports or erases a person from their row in *Members*; erasing names what
  goes and asks for the person's name.
- **API:** `GET /workspaces/{id}/exports` lists a workspace's exports and
  `GET /workspaces/{id}/retention/preview` counts what given periods would remove; the
  `/admin/scim/tokens` routes are in the OpenAPI document. Rotating a revoked SCIM token now
  answers 404 instead of minting a replacement.
- **API:** `GET /channels/{id}/context` returns a channel's members, pins, bookmarks, lists
  and workflow shortcuts in one request. `GET /channels/{id}/messages?mark_read=true` marks the
  channel read along with its newest page.
- **What touches a channel, in the channel.** *Channel details → Integrations* lists the
  incoming webhooks, outgoing webhooks, slash commands and workflows that post into or read
  a channel, for everyone in it. A channel admin adds an incoming webhook there without a
  workspace admin, sees its URL once, and can make a new URL or remove it. The workspace
  *Integrations* section links each hook's channels.
- **Who may notify a whole channel, and who may make one.** *Workspace settings → General*
  has a *Permissions* group: who can build workflows, use `@channel`, `@here` and
  `@everyone`, and create public or private channels, each every member or admins only.
  *Channel details → Settings* turns the three broadcasts off for one channel. The composer
  offers them only to someone who may use them, and in a channel of more than 25 people asks
  *This notifies N people. Send anyway?* first. *Create channel* offers only the kinds the
  person may make, and is gone when they may make neither.
- **API: who may notify a whole channel, and who may make one.** Workspace `settings` take
  `broadcast_mentions`, `public_channel_creators` and `private_channel_creators`
  (`members`, the default, or `admins`) through `PATCH /workspaces/{id}`, and a channel's
  `settings.allow_broadcast_mentions` (on by default) through `PATCH /channels/{id}`. A
  message, poll or scheduled message with `@channel`, `@here` or `@everyone` the author may
  not use is refused with 403 and the reason; one already queued, or sent by a workflow,
  goes out without notifying anyone. In the OpenAPI document `WorkflowBuilders` is now
  `WhoMay`.
- **API:** `GET /channels/{id}/integrations` lists the incoming webhooks, outgoing webhooks,
  slash commands and workflows that touch a channel, for anyone in it, without URLs or
  secrets. A channel admin may now create, reveal, rotate and delete the incoming webhooks of
  their own channel; every other hook still needs a workspace admin.
- **Form answers as fields.** A workflow message with the form answers below it shows them as
  labelled fields in two columns instead of a tall stack of paragraphs; the message text is
  unchanged.

### Changed

- **A message the server refuses says why** (*Only workspace admins and this channel's admins
  can use @channel, @here and @everyone*) instead of *Message failed to send*. A network or
  server failure still says the latter.
- **Channel settings is now *Channel details*,** open to everyone in the channel from ⋯, with
  *About*, *Settings* (moderators) and *Integrations*. The header's *Integration* badge opens
  *Integrations*.
- **Workspace settings in one dialog.** The workspace menu is down to *Invite people*,
  *Workflows* and *Workspace settings*; the last opens one dialog with *General*, *Members*,
  *User groups*, *Custom emoji*, *Apps*, *Integrations*, *Slack import* and *Audit log* for an
  admin, and the four a member uses for everyone else. *General* saves as you change it, like
  *Preferences*. `?settings=<section>` in the address opens a section.
- **Escape closes only the topmost thing**: a confirmation inside a dialog, not both.
- **The invite form shows only to admins**, who are the only ones the server lets invite.
- **The channel header has one members button**, the people count, and the pin button shows
  how many are pinned. *Channel settings* moved into the header's ⋯ menu. Every header icon
  has a tooltip.
- **Workspaces without an icon differ in the rail**: a tile coloured from the workspace and up
  to two letters (*Dev Team* → *DT*, *Design* → *De*).
- **The README lists what ships**, grouped as the app is.
- **Fewer requests.** Signing in makes 24 requests instead of 31, and none of them twice. Opening
  a channel for the first time makes two instead of eight.
- **Workflows have no drafts.** *Create workflow* saves a new workflow and it starts running;
  an existing one has *Save changes*, *Turn off* and *Turn on*. A draft had to be complete to
  save anyway, and once saved it looked exactly like a workflow somebody had turned off.

### Fixed

- **A channel opened on start or from a link is marked read.** Only a click in the sidebar
  used to mark it, so its unread count came back after a reload.
- **A new channel opens once created.** It used to leave you in `#general`, because the app
  went to the channel before its list knew about it.

## [3.3.0] — 2026-10-24

### Upgrading from 3.2.0

- Pull both images and restart `api`, `worker`, `realtime` and `frontend` together; the API
  runs migrations 57–70 on start, so snapshot first. Two upgrade notes in the RUNBOOK cover
  what runs after: [text inside files](docs/RUNBOOK.md#upgrade-note-text-inside-files-becomes-searchable)
  and [lists](docs/RUNBOOK.md#upgrade-note-lists).
- For API clients, three shapes change in a way a strict client notices: `ChannelMember`
  loses `notifications` and `is_muted`, the `notification` frame's `priority` is now `kind`,
  and `GET`/`PATCH /notifications/dnd` answer `{ dnd_until, schedule_paused_until }`. The web
  client in this release already speaks the new shapes. Everything else is added.
- Migration 57 adds the `notify_level` type, `workspace_members.notify_default`,
  `channel_members.notify_level` and `pending_mention_emails.direct`, and drops
  `channel_members.notifications` and `channel_members.is_muted`, which nothing read. The
  `ChannelMember` API shape loses those two fields, and the `notification` frame's
  `priority` is now `kind`.
- Migration 58 adds the `keyword` notification type, the `notification_keywords` table and the
  `keyword_pattern()` function.
- Migration 59 adds `notification_schedules`, `users.dnd_override_until` and the
  `schedule_paused_until()` / `notifications_paused_until()` functions. `GET`/`PATCH
  /notifications/dnd` now answer `{ dnd_until, schedule_paused_until }` rather than
  `{ dnd_until }`.
- Migration 60 adds `files.search_name` (generated) with a trigram index, the `file_kind`
  type and `file_kind_of()`, and `searchable_channels()`, which message search now uses too.
  `idx_files_workspace` becomes `idx_files_workspace_created`.
- Migration 61 adds the `file_texts` table and the `search_headline` text search
  configuration. `chat-worker` then reads the text of existing files in the background; see
  [the upgrade note](docs/RUNBOOK.md#upgrade-note-text-inside-files-becomes-searchable) for
  what that costs and how to watch it.
- Migration 62 adds the `polls`, `poll_options` and `poll_votes` tables.
- Migration 63 adds the `list_column_kind` type and the `lists`, `list_columns` and
  `list_items` tables. The realtime service subscribes to one more Redis channel,
  `events:list`; deploy the backend and realtime images together. A workspace's message
  retention window now also removes list items nobody has edited within it; see
  [the upgrade note](docs/RUNBOOK.md#upgrade-note-lists).
- Migration 64 adds `list_columns.hidden`. `ListColumn` gains `hidden`, and `PATCH
  /list-columns/{id}` takes it.
- Migration 65 adds the `list_view_kind` type and the `list_views` table, and gives every
  existing list a "Table" view. `ListDetail` gains `views`, and a new frame,
  `list.views_changed`, goes to the list's audience; deploy the backend and realtime images
  together.
- Migration 66 adds the `list` notification type, the `list_item_comments` table,
  `list_items.comment_count`, `list_columns.is_due`, `later_items.list_item_id` and the
  `list_due_assignments()` function. `ListItem` gains `comment_count`, `ListColumn` gains `due`,
  `LaterItem` gains `list_item`, and the `notification` frame gains `list_id` and `item_id`.
  Two new frames, `list.comment_upserted` and `list.comment_deleted`, go to the list's
  audience; deploy the backend and realtime images together.
- Migration 67 adds the `workflows`, `workflow_runs` and `workflow_run_steps` tables, the
  `workflow_run_status` type and the `workflow_next_run()` function, and removes `bot` and
  `scheduled` from `hook_type` (nothing ever created either; any such row is deleted).
  `chat-worker` runs one more supervised loop, `workflow_runner`. A message's `bot` identity
  gains `workflow_id`, and its `hook_id` is optional, as slash-command replies already left it
  out. The workspace settings take `workflow_builders`.
- Migration 68 indexes workflows by the channel their trigger listens to. `chat-worker` runs
  `workflow_listener`, a new consumer group (`workflows`) on the workspace streams; like any new
  group it starts at the beginning of each stream, and as nothing listens yet that backlog
  starts no runs. Joining a channel now emits a durable `channel.member_joined` event, which the
  gateway ignores. `WorkspaceService::join_workspace` and `accept_invite` return the events
  they staged.
- Migration 69 adds `waiting` and `expired` to `workflow_run_status` and the `workflow_forms`
  table. A message's metadata gains `form`, and both message steps of a workflow definition an
  optional `summary`. `/workflow` joins the built-in commands, so a registered command of that
  name is shadowed.
- Migration 70 adds `workflow_run_steps.list_item_id`, the item a list step wrote, which a
  `RunStep` in the run history now carries. A workflow definition takes two more steps,
  `add_list_item` and `update_list_item`.

### Added

- **Notification levels.** Each channel can notify about *All new messages*, *Mentions* or
  *Nothing*, from a bell in the channel header and on the sidebar row; every person sets a
  default per workspace in *Preferences → Notifications*. `PUT /channels/{id}/notify-level`,
  `GET`/`PUT /workspaces/{id}/notify-default`.
- **Direct messages reach a phone.** A DM notifies on push, appears in Activity, and is
  counted in the email digest when no device received it.
- **Keyword notifications.** *Preferences → Notifications → My keywords* lists up to 50 words
  or short phrases per workspace. A message with one of them, in a channel you are in,
  notifies you as a mention does (Activity, push, desktop), but without email and without
  moving the mention badge. Case and accents are ignored, whole words only, and the word is
  highlighted in the message for you. `GET`/`PUT /workspaces/{id}/notification-keywords`.
- **Notification schedule.** *Preferences → Notifications → Notification schedule* sets the
  days and hours you want to be notified, in your profile's timezone; a window may cross
  midnight. Outside it, notifications wait in Activity: no push, desktop notification or
  email. *Resume* lasts until the next window opens. The *You* menu now has the pause
  controls and says why notifications are paused, and a member's hover card shows when
  their notifications resume. `GET`/`PUT /users/me/notification-schedule`.
- **Find a file by its name.** Search has a *Files* tab: any part of a file's name finds it,
  ignoring case and accents, in every channel and DM you can read. Narrow by person, place,
  date and *Type* (`type:pdf`). An image opens in the gallery, any other file at its message.
  The channel's *Files* panel filters by name. `GET /search/files`.
- **Find a file by what is inside it.** File search also reads the text of PDFs, Word,
  Excel and PowerPoint files and plain-text files, and shows the matching line under the
  file name with the words marked. A name match still comes first. Text in images is not
  read. `file_text_backlog` and `file_text_extraction_total` on the worker's metrics.
- **Polls.** The poll button next to the paperclip, or `/poll`, opens a dialog: a question, 2 to
  10 options in any order, one answer or any number, anonymous, and an optional closing time.
  `/poll "Lunch?" "Thursday" "Friday"` skips the dialog. The card shows each option's count and
  bar, your own choice, and who voted unless the poll is anonymous; a vote shows at once and
  reaches everybody live. Press a chosen option again to take the vote back. The creator or a
  channel moderator closes a poll, or it closes itself at its time. Works in channels, DMs and
  threads; everybody can vote in an announcement channel. `POST /channels/{id}/polls`,
  `PUT /polls/{id}/vote`, `GET /polls/{id}/votes`, `POST /polls/{id}/close`.
- **Lists, the API.** A list is a table in a channel or DM, or a personal one: typed columns
  (text, number, select, multi-select, person, date, checkbox, link), up to 30 columns and
  10,000 items, checked on the server, merged per cell when two people edit one row, and sent
  live to everyone in the conversation. Exports include each list as CSV. The board and
  comments in the app follow. `/workspaces/{id}/lists`, `/lists/{id}`, `/list-columns/{id}`,
  `/list-items/{id}`.
- **Lists, the table.** *Lists* at the start of a channel's bookmark bar shows the channel's
  lists and *New list*, from blank or a template: project tracker, bug intake, on-call rota. A
  list opens in place of the conversation, at `/app/{workspace}/lists/{list}`. The table is a
  keyboard grid: the arrows move, Enter or typing edits, Escape cancels, Tab moves right,
  Delete clears, Alt + ↑/↓ moves a row and Shift + Enter opens it. Each kind has its own
  editor: a select, a member picker, a date picker, a checkbox that toggles. Rows are
  virtualized, so 10,000 items scroll smoothly; drag a row by its handle to move it. A column's
  header menu edits it (name, kind, choices; a change that clears cells says how many first),
  moves, hides or deletes it. An item opens in a side panel with every field, hidden ones
  included. Edits from others arrive live.
- **Lists, views.** Tabs above a list hold its saved views, shared by everyone who sees the
  list. A *Board* groups items by a single-select column, a lane per choice; drag a card to
  another lane, or press Alt + ←/→ on it, to change its value, and *New* in a lane adds an
  item there. Any view sorts by up to three columns and filters by up to ten rules: text that
  contains, a choice or person that is or is not, numbers and dates before or after, due this
  week, overdue, empty. *Only mine* and *Due this week* are one click. While a table is sorted
  or filtered its rows stay where the view puts them, and items you just added stay in sight.
  `POST /lists/{id}/views`, `PATCH`/`DELETE /list-views/{id}`, `PUT /lists/{id}/view-order`.
- **Lists, comments and due dates.** Every item has a comment thread under its fields, with
  mentions, formatting and edits, live for everyone; a row and a card show how many comments
  it has. Mentioning someone in a comment, or setting a person cell to them, puts it in their
  Activity ("Ana assigned you to *Fix login*"), and the notification opens the item. Mark a
  date column *Use as due date*: an item due tomorrow and assigned to you waits in Later as a
  reminder at 9:00 on the day, and ticking it off there checks the item's checkbox.
  `/list-items/{id}/comments`, `PATCH`/`DELETE /list-comments/{id}`.
- **Workflows.** *Workflows* in the workspace menu lists yours (an admin sees everyone's) and
  builds new ones in a full-height editor: one trigger, then steps in order. A workflow starts
  on a schedule (every day, weekdays or chosen days, at a time in a timezone), when its secret
  webhook URL is called with a JSON object, when someone adds a chosen emoji to a message in a
  channel, when someone joins a channel, or from a shortcut: the ⚡ button in a channel's
  composer, or `/workflow <name>` there. It then posts in a channel or sends someone a DM
  (or the person who set it off), as a message tagged *Workflow*, with the webhook's values,
  that person, the channel, or the reacted message and its link inserted where you put them.
  A form step sends someone a *Fill in* card that opens a dialog of short and long answers,
  choices, people and dates; the run waits for the answers (7 days at most), later steps insert
  them, and a message step can append them all as a summary.
  A list step adds an item to a list, filling its columns from values and variables (a form's
  person, date or choice answer is checked against the column when saving), and a later step
  can set a person, choice or checkbox column on that same item. *New* starts from a blank
  workflow or a template: welcome new members, daily standup reminder, incident from a
  reaction, time-off request into a list.
  Every step runs with what its builder can do at that moment, so losing access to a channel
  fails that step, and a builder leaving the workspace turns their workflows off and tells the
  admins. *Run history* shows the last 50 runs and why a step failed. Admins can keep building
  to admins in *Settings*. `/workspaces/{id}/workflows`, `/workflows/{id}` (with `/publish`,
  `/disable`, `/runs`, `/start`), `POST /workflows/webhook/{token}`,
  `GET /channels/{id}/workflow-shortcuts`, `POST /workflow-runs/{id}/forms/{position}`.

### Changed

- **The server decides what rings.** The browser plays a sound and shows a desktop
  notification only for a `notification` frame, not for every message it receives, and stays
  quiet for the conversation on screen.
- **A mention notifies members only.** Mentioning somebody outside the channel no longer
  notifies them about a message they cannot open.

- **A huddle holds at most eight people.** Past that a peer-to-peer call degrades for
  everyone in it, so the ninth person to press *Join* is told the huddle is full and no call
  starts. Larger calls wait for the SFU in 4.0.0.

### Fixed

- **Uploading from a phone.** *Upload photo* in *Edit profile*, *Upload icon* in workspace
  settings and the composer's paperclip did nothing in the installed app on a phone: they
  opened a hidden file input from script, which an iOS home-screen app ignores. Each is now a
  label the tap goes through to the input itself.
- CI actions: `actions/cache` 6 and `docker/build-push-action` 7. Dependabot no longer
  proposes `dtolnay/rust-toolchain` versions, which it offered before they exist (1.120.0);
  the toolchain moves by hand with `rust-version` and the build image.

## [3.2.0] — 2026-10-23

### Upgrading from 3.1.0

- Nothing is required: there are no migrations and the API is unchanged, so only the frontend
  image differs in behaviour. The sign-in fields no longer carry the ids `email` and
  `password`; a script that signs in through the page should select
  `[data-qa="login-email"]` and `[data-qa="login-password"]` instead.

### Changed

- **Dialogs look finished.** Every dialog is one of three widths, keeps its title and buttons
  in place while the middle scrolls, and fades in and out (not under *Reduce motion*); on a
  phone it is a sheet from the bottom. Every field in the app has a label, one height and one
  text size, and checkboxes and selects look like the app rather than the operating system.
- **Create channel** chooses public or private from two cards, shows the name with its `#` and
  a counter to the 80-character limit, and turns spaces into hyphens and capitals into lower
  case as you type. Existing names are left alone and the API accepts what it did before.
- **Browse channels** is large, counts what it shows, sorts by name, members or age, has
  *Create channel* in its header, and gives each row one action, *Join* or *Leave*, on hover or
  focus.
- **Confirmations.** Deleting a message shows the message, archiving a channel names it, and
  deleting an integration names it, each in a small dialog where Enter confirms and Esc
  cancels.
- **Profile and Preferences are two dialogs.** *Edit profile* holds your name, status (with
  presets and *Clear after*), bio and photo, saved with one button. *Preferences* lists
  *Notifications*, *Appearance* and *Security* on the left and shows one at a time; each
  setting saves as you change it. The *You* menu has both.

## [3.1.0] — 2026-10-23

### Upgrading from 3.0.0

- Migration 56 follows every existing thread for its root's author and everyone who replied
  in it, read up to the moment of the upgrade, so nothing arrives unread. The realtime service
  subscribes to one more Redis channel, `events:thread`; deploy the backend and realtime images
  together. RUNBOOK → *Upgrade note: followed threads*.

### Added

- **Keyboard shortcuts, as in Slack.** ⌥↑ / ⌥↓ move through the sidebar's channels and
  conversations, ⌥⇧↑ / ⌥⇧↓ through the unread ones; Esc marks the open channel read and ⇧Esc
  marks everything in the workspace read (`POST /workspaces/{id}/read`). ↑ in an empty message
  box edits your last message. Messages are one Tab stop: ↑ / ↓ walk them, and on the focused
  one `E` edits, `T` or Enter opens the thread, `R` reacts, `A` saves for later, `P` pins and Esc
  goes back to the message box. ⌘/ (Ctrl+/) lists every shortcut.
- **Search filters.** The search panel has *From*, *In*, *Has* and *Date* chips, and the same
  filters typed into the box (`from:@ana in:#design has:file before:2026-10-01 after:… on:…`)
  turn into chips as they are recognised. `GET /search` gains `has` (`file`, `image`, `link`),
  `after` and `before`, takes a DM in `channel_id`, and runs on filters alone when there are no
  words.
- **Threads.** A *Threads* entry under *All unreads* lists the threads you follow, newest reply
  first, each with its root, the latest replies, *N new replies* and a reply box in place; the
  entry counts the threads with unread replies. You follow a thread when you start it and
  somebody replies, when you reply, and when you are mentioned in it; the thread panel has
  *Follow* / *Unfollow*. A reply in a followed thread sends the `reply` notification, in-app
  and push. New routes: `GET /workspaces/{id}/threads`, `GET /workspaces/{id}/threads/unread`,
  `POST /threads/{id}/read`, `PUT` / `DELETE /threads/{id}/follow`.

## [3.0.0] — 2026-10-22

### Upgrading from 2.4.0

- **Major release: the saved-message and reminder routes are gone.** Migration 55 moves saved
  messages and reminders into the new *Later* list and drops their old tables; delivered
  reminders arrive as already fired, and nothing else is required. The
  `/workspaces/{id}/saved` and `/workspaces/{id}/reminders` routes are replaced by
  `/workspaces/{id}/later` — anything scripted against them needs the new route. Take a
  backup first: the old tables do not come back on a downgrade. RUNBOOK → *Upgrade note: Later
  replaces saved messages and reminders*.

### Added

- **Later, Slack's to-do list.** Saved messages and reminders are one list with *In progress*,
  *Completed* and *Archived*: tick an item off, archive it, give it a due time (in 20 minutes,
  an hour, three hours, tomorrow, next week or any time), and it comes back as a reminder and
  stays until it is done. *Later* sits under *All unreads* with a count; a message shows when
  it is saved, and *Remind me about this* saves it with a due time in one step. `/remind`
  lands there too.

### Changed

- **Buttons, panels and dialogs look and behave the same everywhere.** One button, icon button,
  spinner, panel header and dialog header replace the dozens of hand-styled copies, so text
  sizes and edges line up (the *Browse channels* footer among them), every dialog closes from
  a ✕ top-right, every icon-only button has a name for screen readers and a target large
  enough for a thumb, and titles are in sentence case.

## [2.4.0] — 2026-10-22

### Upgrading from 2.3.0

- Nothing is required, but migration 52 rewrites old messages: every `[file: …](…)` link that
  pointed at the message's own upload moves out of the text into the message's attachments.
  A message that was only a file keeps no text. Quotes of other people's files in forwards
  stay as links. Take a backup first, as before any upgrade.
- Migration 53 adds image sizes. Uploads of images now use up to 30 MB of memory each while
  they are processed.
- Migration 54 adds indexes on `files` that downloads and attachments were missing; on a large
  instance it takes a moment.

### Changed

- **Files go out with the message, like in Slack.** Picking, pasting or dropping files puts
  them in a tray above the composer with a progress bar each, where they can be removed; Send
  posts the text and up to ten files as one message. A pasted screenshot is never sent unseen.
  Several images show as a grid with *+N*, other files as cards with their type and size.
  Editing can remove a file, forwarding carries the files, a scheduled message can carry them,
  and the composer's size limit is the server's instead of a fixed 25 MB.
- **Images load as thumbnails and the list stops jumping.** Uploaded images get a thumbnail and
  their size, so a message reserves the picture's space before it loads; the original opens in
  the preview. Photos are stored the right way up and **without their location**: EXIF and
  other metadata are removed on upload (the pixels are not re-encoded). Images uploaded before
  this release keep loading in full.
- **File names stay as they were written.** *Izveštaj o radu.pdf* is stored, shown and
  downloaded under that name instead of *Izve_taj_o_radu.pdf*; names with any letters, spaces
  and punctuation survive, and a download carries the name in UTF-8. Files uploaded before this
  release keep the names they were stored with.
- **The image preview is a gallery.** ← / → (or a swipe on a phone) steps through every image in
  the conversation or thread, with who sent it and when, a counter, click to zoom, *Open
  original* and *Download*.
- **Videos and audio play in the message.** MP4, WebM and the like get a player, voice notes
  and music an audio player, and both can seek: downloads now stream from storage and answer
  byte ranges, so a large file no longer costs its own size in server memory per download.
- **A channel's files.** *Files* in the channel header lists everything shared there — images as
  a grid that opens the gallery, everything else by type with who shared it and when — filtered
  by kind, each leading back to its message.

## [2.3.0] — 2026-10-22

### Upgrading from 2.2.0

- Nothing is required. Migrations 49–51 add the prompt opt-outs and allow the Figma and
  Confluence apps, which stay off until a workspace admin sets them up under *Apps*.
- The frontend image's Content-Security-Policy gains `frame-src` for the YouTube
  (`youtube-nocookie.com`), Vimeo and Loom players. A reverse proxy that sets its own CSP
  needs the same three hosts for inline video.
- New optional settings `FIGMA_WEB_URL` and `FIGMA_API_URL` exist only for an egress proxy.

### Added

- **Figma cards.** Designs, prototypes, FigJam boards and Slides show their thumbnail, project
  and last edit, fetched with the poster's Figma account. Setup is a private Figma app; see
  RUNBOOK → *Connected apps*.
- **Confluence cards.** Pages and blog posts show their space, owner and last edit. Confluence
  can reuse the Jira app's credentials, and each person connects it separately, so a Jira
  connection never reads Confluence.
- **Videos play inline.** YouTube, Vimeo and Loom links show the video's thumbnail and play in
  the message on click.
- **Never ask me again.** The *Connect* prompt under a link now offers *Connect*, *Not now* and
  *Never ask me again*; the last turns that app's prompts off for you, and *Apps* turns them
  back on.
- **Apps as a directory.** Every app shows its icon, what it previews and whether you are
  connected; cards and prompts carry the same icon, and app cards show a picture when the
  service has one.

## [2.2.0] — 2026-10-21

### Upgrading from 2.1.0

- Nothing is required. Migration 48 adds the connected-app tables; Jira, GitHub and Google
  Drive stay off until a workspace admin sets them up under *Apps* (RUNBOOK → *Connected
  apps*). Public GitHub links get cards straight away.
- Before setting an app up, check that `PUBLIC_URL` is the address people open the app at:
  the callback URL each service is given is built from it.
- App secrets and everyone's app tokens are encrypted with a key derived from `JWT_SECRET`.
  Changing `JWT_SECRET` after apps are set up means setting them up again and everyone
  reconnecting.
- New optional settings `ATLASSIAN_*_URL`, `GITHUB_*_URL` and `GOOGLE_*_URL` exist only for an
  egress proxy; leave them unset otherwise.

### Added

- **Jira cards.** A workspace admin connects Jira Cloud under *Apps* by following the steps on
  screen; each person connects their own Atlassian account, and their Jira links then show
  the issue's type, status, priority and assignee. Cards are fetched with the poster's
  account, so nobody sees an issue through a card that they could not open themselves.
  Setup is in RUNBOOK → *Connected apps*.
- **GitHub cards** for pull requests (open, draft, merged or closed, author, size, labels),
  issues, commits and repositories. Public repositories need no setup; private ones need a
  read-only GitHub App that an admin sets up under *Apps*, and each person connects their own
  GitHub account.
- **Google Drive cards** for Docs, Sheets, Slides, Forms, Drawings, files and folders: type,
  owner, last modified and by whom. The app can read file metadata only, never contents.

### Fixed

- A link written as `<https://…>` — the way Slack and most tools paste one — shows as a link
  instead of keeping its angle brackets.
- A link to a page behind a login (a Jira ticket, a private document) no longer gets a card
  that says only the site's name; previews need a description or an image to be shown.

## [2.1.0] — 2026-10-21

### Upgrading from 2.0.0

- **On the `s3` profile, MinIO is gone and RustFS takes its place.** After the upgrade starts
  RustFS, copy existing uploads across once with `docker/migrate-minio-to-rustfs.sh`; until
  you do, attachments uploaded before the upgrade do not open. Nothing is deleted — the old
  MinIO volume stays where it was. Steps in RUNBOOK → *Upgrade note: RustFS replaces MinIO*.
  If you set `S3_ENDPOINT=http://minio:9000` yourself, change it to `http://rustfs:9000`.
- Migration 47 adds the sidebar-section tables and a nullable column; nothing to do.
- The worker now makes outbound HTTPS requests for link previews. A workspace that does not
  want that turns previews off in its settings.

### Added

- **Link previews.** Links in a message get a card with the page's title, description, site
  and image, fetched by the worker. The author can remove a message's previews; a workspace
  admin can turn them off in workspace settings.
- **All unreads**: every channel and DM with new messages on one page, muted ones last, each
  with its newest messages and a *Mark as read* button. In the sidebar and on Shift+Alt+A;
  Esc marks the focused channel read.
- **Sidebar sections** of your own: create, rename, reorder and delete them, and move channels
  in by drag and drop or the channel's *Move to…* menu. Your layout, including which sections
  are collapsed, follows you to every device.
- **Mark as unread** from a message's menu, or Alt+click. The channel's badge comes back with
  the right count and stays until you read past that message again.

### Changed

- Reading or marking a channel unread updates its badge on your other tabs and devices
  straight away; before, they caught up on the next reload.
- **RustFS replaces MinIO** for file storage on the `s3` profile, and `rclone` replaces `mc`.
  MinIO's images can no longer be pulled. Existing uploads have to be copied once with
  `docker/migrate-minio-to-rustfs.sh`; see the RustFS upgrade note in RUNBOOK.

## [2.0.0] — 2026-10-21

Everything since 1.0.0: thirteen waves of security, compliance, reliability and product work,
and the first release measured at 500,000 messages.

### Upgrading from 1.0.0

**Take a backup, then read every upgrade note in RUNBOOK from Wave 1 onwards before you
start.** Migrations run forward on startup and migration 45 is not reversible. The ones that
change behaviour you will notice:

- Outstanding invites expire (Wave 1) and outgoing webhooks are deactivated until an admin
  scopes them to channels (Wave 4).
- Direct messages became channels of type `dm` / `group_dm`; the `/conversations/…/messages`
  routes and `conversation.message.*` frames are gone, so deploy api, worker, realtime and
  frontend together.
- The worker now delivers email and needs the same `SMTP_*` settings as the api.
- Dumps taken before this release do not restore cleanly; take a fresh backup straight after
  upgrading.
- Postgres needs `shm_size: 256mb`, which the compose files now set.

### Added

- **Slack import** from the app or the `chat-import` CLI: channels, private channels, DMs,
  threads, reactions, pins, files and custom emoji, matched to people by email, resumable and
  dry-run first.
- **Threads in DMs**, saved items, channel bookmarks, message forwarding, a status you set
  yourself, reminders and `/remind`, scheduled send.
- **Huddles** with screen sharing, ringing and a TURN relay.
- **Web Push** to a closed app, app-icon unread badges, mention emails for mentions nobody saw.
- **Custom emoji, user groups, bots, slash commands** and incoming webhooks that post as
  themselves.
- **Announcement channels**, guest containment (guests see only the people they share a
  channel with), channel settings and a channel browser.
- **SSO (OIDC), TOTP with recovery codes, SCIM deprovisioning.**
- **Audit log, retention policies, per-user export and erasure.**
- A responsive layout and installable PWA; profile cards on hover; named reactors.
- Published container images and this changelog.

### Changed

- Access control lives in one module and session revocation in one path; membership changes
  reach open sockets immediately.
- Real-time delivery is durable: Redis Streams with replay, an event outbox written in the
  same transaction as the row, and consumer groups for the worker.
- Background work runs in its own `chat-worker` process.
- The message list renders without an editor per message and is virtualized; unread counts
  and presence no longer scan.
- The API contract is generated: OpenAPI for HTTP, a generated union for WebSocket frames.
- Search reaches DMs, folds diacritics and tolerates typos. It ranks the newest 1,000 exact
  matches, uses fuzzy matching only when exact matching runs short, and plans each query for
  its term: 7–158 ms p95 at 500,000 messages, down from up to 13 s.

### Fixed

- Backups restore completely: the search functions are schema-qualified, and the restore
  procedure stops on the first error and analyzes afterwards.
- Every finding of the 2026 security review: invite lifecycle, attachment access for DMs,
  guest scoping in search, upload size enforcement, write and WebSocket rate limits,
  fail-closed auth limiting, constant-time login, scheduled messages re-authorized at
  delivery, input validation gaps.

### Security

- Dependencies current as of 2026-10-20 with no open `npm audit` or `cargo audit` advisories;
  both gates block CI.

## [1.0.0] — 2026-06-05

First public release.
