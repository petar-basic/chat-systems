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
