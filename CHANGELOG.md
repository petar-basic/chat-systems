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

### Upgrading

- `POST /workflows/{id}/publish` is now `POST /workflows/{id}/enable`, and
  `POST /workspaces/{id}/workflows` creates a workflow that is already on. A workflow saved as
  a draft before the upgrade is one that is off; *Turn on* starts it.

### Changed

- **Workflows have no drafts.** *Create workflow* saves a new workflow and it starts running;
  an existing one has *Save changes*, *Turn off* and *Turn on*. A draft had to be complete to
  save anyway, and once saved it looked exactly like a workflow somebody had turned off.

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
