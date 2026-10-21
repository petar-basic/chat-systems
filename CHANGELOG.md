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
