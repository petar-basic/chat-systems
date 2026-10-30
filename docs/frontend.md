# Frontend

A React + Vite single-page app. State is split between **TanStack Query** (server state)
and **Zustand** (UI state); WebSocket events flow through a typed **global event bus** that
reconciles them into the Query cache.

## Layout

Every file under `frontend/src/` is in one of these places. `npm run lint` refuses a file
anywhere else (`eslint-rules/feature-structure.js`).

| Folder | What lives there |
|---|---|
| `features/<feature>/` | Everything one part of the product needs: its components, its query and mutation hooks in `hooks/`, and its helpers. Tests sit beside the file they test. The features are `channel`, `huddle`, `instance` (sign-in, setup, instance admin), `later`, `lists`, `messaging`, `navigation`, `notifications`, `polls`, `profile`, `search`, `setup`, `threads`, `unreads`, `workflows` and `workspace` |
| `features/<feature>/index.ts` | What other features may use. A feature imports another only as `@/features/<name>`. The lint rule refuses `@/features/<name>/hooks/...` or a relative path into another feature, except in tests, which may mock a module directly |
| `shared/` | What several features use and none owns: the controls in `shared/components` (Button, Modal, Form, Popover, Avatar, PresenceDot, ErrorBoundary…), `shared/hooks`, `shared/constants` (query keys, routes, labels) and `shared/models` |
| `pages/` | One component per route, composing features |
| `lib/` | Infrastructure with no UI: the API client, the WebSocket, the instance manager, the event bus and the cache reducer, plus pure helpers (markdown, emoji, mentions, dates) |
| `stores/` | Zustand stores for UI and session state |
| `api/` | Generated from the backend: `schema.d.ts` from the OpenAPI document and `serverFrames.ts` from the gateway's frames |
| `test/`, `types/` | Vitest setup and mocks; ambient type declarations |

Choose the folder by what the code is about, not by its kind. A query hook for pins lives
in `features/channel/hooks`, not in a shared hooks folder. A component that only messaging
renders lives in `features/messaging`, even when it is small and general-looking.

## Architecture & Rationale

### Why this stack

- **Vite SPA, not Next.js.** The app is a pure client behind a Rust API — there's no SSR
  need, no SEO surface (it's invite-only and auth-gated), and a long-lived WebSocket drives
  most updates. A Vite SPA keeps the build simple and the runtime a static bundle any nginx
  can serve. (It is *not* a Next.js / Server-Components app; don't read it as one.)
- **TanStack Query for server state, Zustand for UI state.** Server data (messages,
  channels, members) wants caching, dedup, retries, and optimistic updates — Query's job.
  Ephemeral UI state (current selection, open panels, presence, drafts) is plain client
  state — Zustand, no boilerplate. Keeping them separate avoids the "everything in one
  global store" trap.
- **The contract is generated, twice.** `src/api/schema.d.ts` comes from the backend's
  OpenAPI document and every request goes through `apiClient.typed(...)`, so a path, body or
  response the server does not have fails `tsc`. `src/api/serverFrames.ts` is generated from
  the gateway's `ServerFrame` enum; `wsQuerySync` is the single reducer that turns each frame
  into the right cache mutation (upsert/patch/remove), so realtime updates and HTTP responses
  converge on one source of truth.

### Structure

- Smart logic lives in hooks and views stay thin. For example, `useWorkspaceController` holds
  the page's data and handlers, and `WorkspacePage` only renders.
- A component that grew more than one job is split along those jobs. `MessageItem` is the
  row, and `MessageActionsBar`, `MessageBody`, `MessageEditor` and `MessageReactions` are its
  parts. `HuddleWindow` is the frame around `HuddleStage`, `VideoTile` and
  `HuddleControlBar`.

### Cross-cutting decisions

- **Bearer auth, transport varies by origin** — `ApiClient` holds the access token in memory
  and refreshes it (see below). `fetch` always uses `credentials: 'include'` (so same-origin
  cookies ride along), and cross-origin requests additionally send `Authorization: Bearer`.
  The WS upgrade carries the token as the `bearer` subprotocol.
- **Strict TypeScript** — no `any`, `@ts-ignore`, or `eslint-disable`; server events are a
  discriminated union driving a type-safe bus.
- **Query keys go through a single `QUERY_KEYS` factory** (`shared/constants`), so a query and
  its invalidation can never silently drift apart.
- **Message ordering is self-healing** — `messageCache.upsertMessage` re-sorts the target page
  with the supplied comparator (`newestFirst`, by `created_at`), so out-of-order or
  late-confirmed realtime messages still render correctly.
- **Code-split by route** — pages are `React.lazy()`, so login/reset/add-instance don't pull
  in the heavy editor/messaging chunk.
- **Installable PWA** — `public/manifest.webmanifest` + icons make the SPA installable
  (standalone window); unread counts go to the app icon via the Badging API
  (`navigator.setAppBadge`) and to the favicon, and desktop notifications use the
  Notification API. The server decides who is notified: the client rings and shows a desktop
  notification only for a `notification` frame, and only when the person is not already
  looking at that channel or conversation. The app badge is always the total of unread
  Activity across every workspace; a push sends the same total for its instance.

### The read rule

Unread state and Activity follow one rule, so the sidebar badge and the Activity row clear
together:

| Notification about | Clears when | Sidebar badge it pairs with |
|---|---|---|
| A message in a channel or DM | That channel or DM is read (opened, *Mark read*, *Mark all read*) | The channel's or DM's unread / @ count |
| A reply in a thread | That thread is read (opened in the panel or in *Threads*) | *Threads* unread |

- **On screen counts as read.** A message that arrives in the channel or DM open in a focused
  window is read straight away (`readOnScreen`), and so is a notification about it that lands
  after the message; a reply that arrives in the open thread re-reads that thread.
- **A thread reply is not a channel message.** It does not move the channel's unread or @
  count unless it was also sent to the channel, and reading the channel leaves its
  notification alone.
- **Every realtime update checks its workspace.** `message.new`, `notification` and
  `channel.read_state` frames for another workspace never touch the open workspace's sets;
  they only refresh that workspace's counts.
- **Cross-tab.** Every read publishes `channel.read_state` or `thread.read_state`; a tab that
  sees one with nothing unread refetches its Activity list and count.

## Authentication Model

Login/refresh return `{ access_token, refresh_token, user, expires_in }` in the JSON body, and
the backend also sets them as `HttpOnly; SameSite=Lax` cookies. The frontend is **bearer-first**:
`ApiClient` keeps the access token in memory, decodes its `exp`, and refreshes proactively (10s
before expiry) or reactively on a 401. How the token reaches the server and how the refresh token
is persisted depends on origin:

| Context | Access token | Refresh token | Sent as |
|---------|-------------|---------------|---------|
| Same-origin web | in memory | server cookie (`/api/auth`) | cookie (`credentials: 'include'`) |
| Cross-origin web | in memory | `localStorage` (`chat_tokens`, per-URL) | `Authorization: Bearer` header + WS `bearer` subprotocol |

**Consequences for the frontend:**
- All `fetch` calls use `credentials: 'include'`; cross-origin/desktop also attach `Authorization: Bearer <access>`
- `WebSocketClient` resolves a valid token (`api.getValidToken()`) and opens the socket with `['bearer', token]` as the WS subprotocol; if no token is available it connects without one (relying on the cookie)
- On 401, `ApiClient` refreshes once (single-flight) then retries; same-origin refresh is a cookie-only `POST /auth/refresh`, cross-origin sends the stored refresh token as a bearer header. If refresh fails, `onSessionExpired` removes the instance.
- The backend requires `CORS_ORIGINS` (comma-separated allowed origins) and validates the WS upgrade `Origin` against it; wildcard `*` is not allowed when credentials are enabled
- `localStorage` (`chat_instances`) stores only `{ url, wsUrl?, user }` per instance — no tokens for same-origin; cross-origin tokens live in the separate `chat_tokens` map

---

## Pages

### `WorkspacePage`
Main app container, kept thin: it calls `useWorkspaceController()` for all data and
handlers and renders the layout (sidebars, conversation, right panels). The logic lives
in the hook, not the view. It is mounted by several routes that all resolve to the same
page: `/app/:workspaceId?`, `/app/:workspaceId/:channelId`,
`/app/:workspaceId/:channelId/:messageId`, `/app/:workspaceId/c/:conversationId` and
`/app/:workspaceId/lists/:listId`, which renders `ListView` in place of the conversation and
leaves the selected channel as it was.
A conversation is a channel of type `dm`/`group_dm`, so with a `conversationId` the page
renders `ConversationView` — a header with the participants over the same `MessageList`,
`MessageInput`, `TypingIndicator` and `ThreadPanel` a channel uses, keyed on the
conversation id as the channel id.

- **`features/workspace/hooks/useWorkspaceController`** — composes the page's hooks and
  returns one object the view consumes: `useWorkspaceData` (queries, store hydration,
  socket joins), `useWorkspaceRouting` (URL↔store sync), `useComposer` (send, slash
  commands, uploads, typing) and `useMessageActions` (save, forward).
- **`features/workspace/hooks/useRightPanel`** — open-panel state (members / settings /
  thread / search / pins / channel members / notifications), Esc-to-close, reset on
  conversation change.
- **`features/workspace/WorkspaceRightPanels`** — presentational; renders the active panel.

---

### `AddInstancePage`
Login page for connecting a new instance (optional separate WS URL field for split deployments).

**Reads:** form input (instance URL, email, password, optional WS URL)
**Writes:** `useInstanceStore.addInstance()` → POST `/auth/login` → stores `{ url, wsUrl?, user }` in `chat_instances`; access token kept in memory, refresh token persisted per the table above

---

### `LoginPage` / `CompleteRegistrationPage` / `ResetPasswordPage`
Standard auth flows. POST to their respective `/auth/*` endpoints.

---

### `InstanceAdminPage`
Instance-admin user management for the active instance (route `/app/admin`). Instance-admin
gated; redirects non-admins to `/app`.
**Reads:** GET `/admin/users?limit=200`, GET `/admin/audit-log?limit=200` (through `features/instance/hooks/useAdmin.ts`)
**Writes:**
- POST `/admin/users/{id}/suspend` / `/admin/users/{id}/activate` — toggle account status
- PATCH `/admin/users/{id}/instance-role` `{ is_instance_admin }` — grant/revoke instance admin

(Workspace deletion lives in *Workspace settings → General*, not here.)

---

## Features

### `workspace/WorkspaceSidebar`
Vertical sidebar showing workspaces grouped by instance.

**Reads:** workspaces list, `useInstanceStore`
**Emits:** `onSelectWorkspace()`, `onCreateWorkspace()`, `onAddInstance()`

---

### `channel/ChannelSidebar`
Vertical sidebar showing channels, DMs, and workspace members with presence dots.

**Reads:** channels, workspace members, `usePresenceStore` (online/away/offline), `useUserCache`
**Emits:** `onSelectChannel()`, `onOpenMembers()`, `onOpenSettings()`, `onOpenProfile()`, `onLogout()`

---

### `channel/ChannelHeader`
Top bar with channel name, topic, the huddle and notification controls, and toggles for
members (the people count), pins (with their count), files and search. *Channel details*
sits in the ⋯ menu for everyone in the channel; below `sm` the toggles fold into it. The
*Integration* badge, shown while an outgoing webhook reads the channel, opens it on
*Integrations*.

### `channel/details/ChannelDetailsModal`
A `SectionedModal` with *About* (name, topic, description and archive for a moderator, the
topic and description as text for everyone else), *Settings* (announcement channel, whether
`@channel`, `@here` and `@everyone` work there, and message retention, moderators only) and
*Integrations*. *Integrations* reads
`GET /channels/:id/integrations`: the incoming webhooks bound to the channel, the outgoing
webhooks and slash commands scoped to it, and the workflows that start from it or post into
it. When the server says `can_add_webhook`, the viewer adds an incoming webhook, makes a new
URL for one or removes it; a URL is shown once, under its webhook, after it is made. A
workflow row opens the workflow in the *Workflows* panel for someone who may build them
(`useRightPanel().openWorkflow`). Archiving closes the modal and leaves the person in the
channel, now read-only.

### `channel/ChannelArchivedBar`
Takes the composer's place, ahead of the preview bar and the announcement notice, while the
current channel has `is_archived`. *Unarchive* is there for a channel moderator
(`useChannelModeration().canModerate`). `lib/channelArchive.applyChannelArchived` patches the
open channel and its `channel` query and refreshes the channel list and *Browse channels*; the
archive and unarchive mutations call it, and so does `wsQuerySync` on `channel.updated`, so
everyone viewing the channel switches at once. *Browse channels* lists archived channels with
*Archived* in place of *Joined*; a row opens the channel to read, with no *Join* or *Leave*. The
thread panel of an archived channel says so in place of its composer.

**Reads:** current channel
**Emits:** `onToggleSearch()`, `onTogglePins()`, `onToggleFiles()`, `onToggleChannelMembers()`

---

### `messaging/MessageList`
Infinite-scrolling, virtualized message list; newest messages at the bottom. Scrolling to top triggers `fetchNextPage()`.

Opening a channel with unread messages lands on the first of them under a *New* divider, loading
older pages until the last read message is in the list. The boundary comes from the sidebar's
unread counts when they are loaded, and otherwise from the first page's `last_read_before` (the
server reports where the reader stopped when that page marks the channel read), so a cold load
draws it too. *Jump to first unread* shows while the divider is above the view and the end of
the channel is not; *New messages* shows when something arrives while the reader is above the
end.

**Reads:** `useMessages(channelId)` — infinite query, cursor-based pagination, 50 messages per page
**Emits:** `onThreadOpen(message)` when user clicks reply icon

---

### `messaging/MessageInput`
Text input with file attachment button and send button.

**Reads:** channel name
**Emits:** `onSend(content)`, `onFileUpload(file)`, `onTyping()` on each keystroke

Given a `pollTarget` (the channel, DM or thread it writes to), it has a poll button next to the
paperclip and reads `/poll` before sending: `/poll` alone opens `PollDialog`,
`/poll "Q" "A" "B"` creates the poll straight away (`parsePollCommand` in
`features/polls/pollCommand.ts`), and a malformed one is a toast, not a message.

---

## Components

### Shared controls (`shared/components`)

Every screen builds its controls from the same few pieces, so sizes, spacing and focus rings
line up everywhere:

- **`Button`** — `variant` `primary` · `secondary` · `ghost` · `danger`, `size` `sm` (28 px) ·
  `md` (36 px), `loading` (spinner, disabled, `aria-busy`), `icon`, `fullWidth`. A link that
  must look like a button uses `buttonStyles()` from the same folder.
- **`IconButton`** — a 28 or 32 px target around a 16 px icon; `label` is required and becomes
  the accessible name and the tooltip; `pressed` for toggles; `tone="inverse"` on dark overlays.
- **`Spinner`** — `sm` · `md` · `lg`, in the current text colour.
- **`PanelHeader`** — every right panel's 56 px header: icon, title, optional badge and
  actions, and a close button labelled *Close {title}*.
- **`Modal`** — `size` `sm` (400 px) · `md` (520) · `lg` (720); a fixed header (title,
  `description`, `headerActions`, ✕) and footer (`footerStart`, `dismissLabel`, `actions`)
  around a body that is the only scroll area; `onSubmit` makes the whole dialog a form.
  **`SectionedModal`** builds a dialog of sections on it: a vertical tab list (arrow keys,
  Home/End) beside one section at a time; *Preferences* and *Workspace settings* use it. On a
  phone it is a sheet from the bottom. **`Overlay`** under it holds the portal, focus trap,
  initial focus (`data-autofocus`, then the first field in the body) and the exit fade, which
  runs for the ✕, Escape, the backdrop and the dismiss button.
- **`ConfirmDialog`** — title, `description`, a slot for what is about to go, and a `danger` or
  `primary` action that gets focus, shows busy while it runs and keeps the dialog open with the
  reason if it throws.
- **Form** (`shared/components/Form`) — `TextInput` (`leading`, `trailing`, `counter`),
  `TextArea`, `Select`, `Checkbox` (with `description`) and `RadioCards`, all 36 px (`compact`
  32 px for rows and toolbars), each wired to its label, hint, error and counter by `Field`.
  `labelHidden` keeps the label for screen readers only. `FilePicker` is a `<label>` styled
  as the button, around a visually hidden file input: the tap itself opens the picker. A
  `display: none` input opened with `ref.current.click()` is an untrusted click, and an iOS
  home-screen app ignores it.

Titles and labels are sentence case. ESLint enforces the rest outside `shared/components`: a
hand-styled primary button (`hover:bg-purple-500`), a spinner (`rounded-full animate-spin`)
or a raw `<input>`, `<textarea>` or `<select>` (file pickers excepted) fails
`no-restricted-syntax`, as does `ref.current.click()`, and an icon-only `<button>` without `aria-label` fails
`local/icon-button-label` (`eslint-rules/`).

### `MessageItem`
Single message row. The hover toolbar is `MessageActionsBar`. Content, polls, forms and link
previews are in `MessageBody`. Edit mode is `MessageEditor` and reactions are `MessageReactions`. The
row keeps focus, keys, the sender line and the delete confirmation. It does not
call the API directly — actions go through `useMessageActions`, which wraps the message
mutation hooks (`useEditMessage`, `useDeleteMessage`, `useReactToMessage`,
`useRemoveReaction`, `usePinMessage`).

**Reads:** message, `useCurrentUser`, `useUserCache`, `useWorkspaceStore`
**Endpoints hit (via hooks):**
- PATCH `/messages/{id}` — edit
- DELETE `/messages/{id}` — soft delete
- POST `/messages/{id}/reactions` — add reaction
- DELETE `/messages/{id}/reactions/{emoji}` — remove reaction
- POST `/messages/{id}/pin` (pin) / DELETE `/messages/{id}/pin` (unpin)

A message with `metadata.poll` renders `PollCard` instead of its text and cannot be edited;
digit keys 1–9 on the focused row vote for that option.

---

### `polls/PollCard` and `polls/PollDialog`
`PollCard` draws a poll message: the question, one `aria-pressed` button per option with its
bar, count and your own choice, up to three voter avatars unless the poll is anonymous, and a
footer with the vote count and *closes …* or *Closed*; the creator gets *Close poll*. A vote is
optimistic: `useVote` moves your choice and the counts in every cache that holds the message,
and puts both back if the server refuses. Your own choice is `GET /polls/{id}/vote`, the
avatars `GET /polls/{id}/votes`, refetched when the counts change. `PollDialog` is a CS-059
`Modal`: question, 2–10 options (drag, or Alt + ↑/↓, to reorder), one answer or any number,
anonymous, and an optional closing time.

**API calls:** POST `/channels/{id}/polls`, GET/PUT `/polls/{id}/vote`, GET `/polls/{id}/votes`,
POST `/polls/{id}/close`

---

### `lists/ListView`, `ListGrid`, `ListBoard`, `ItemPanel`
`ChannelListsMenu` sits first in the channel's bookmark bar: the channel's lists, and *New
list* (`NewListModal`, a title and a template from `templates.ts`) for whoever may post there.
`ListView` is the route view: the title (click to rename), the channel it lives in, the item
count, *N hidden* to show hidden columns again, and *Delete list* for its creator or a channel
moderator. The open item is `?item=` in the url, so it survives a reload, and so is the
open view, `?view=` (the first view without it).

`ViewTabs` is a tablist of the list's saved views (←/→ between them; a menu renames, moves and
deletes the active one; *+* adds a table or a board). `ViewControls` holds a board's *Group by*
and the *Sort* and *Filter* popovers; each change saves the view for everyone at once
(`PATCH /list-views/{id}`, optimistic). A filter row that is still missing its value stays a
draft in the popover and is not sent. `viewModel.ts` does the work in the client:
`usableConfig` drops rules whose column is gone or no longer fits, `applyView` filters (every
rule must hold, `"me"` is the viewer, *this week* is Monday to Sunday in local time) and sorts
(empty cells last in either direction, rank breaks ties). Items created in this session stay
in sight even when the filter would hide them. While a table is sorted or filtered,
`ListGrid` drops its row grips and Alt + ↑/↓, since rank is not what orders it.

`ListBoard` lays a lane per choice of the grouping column, then *No <column>*, each listing 50
cards at a time. A card is a button: Enter opens the item, the arrows walk cards, and Alt +
←/→ or dragging it to another lane writes the new choice through the same optimistic cell
edit the table uses. *New* in a lane creates an item with that choice and opens it.

`ListGrid` is an ARIA grid that keeps focus on itself and points `aria-activedescendant` at the
active cell, so the virtualized rows (`@tanstack/react-virtual`, fixed 36 px rows, the header
as `scrollMargin`) can come and go without losing focus. Arrows, Home/End (with Ctrl, the first
or last row), PageUp/PageDown move; ↑ from the first row reaches the column headers, where Enter
opens the column menu. Enter, F2 or a typed character edits; Tab commits and moves right,
wrapping to the next row, and leaves the grid from the last cell; Escape cancels; Delete
clears; Alt + ↑/↓ moves the row; Shift + Enter opens the item. The active cell and the cell
being edited are kept by item and column id, not index, so a row arriving above from somebody
else never moves an edit to another item. `CellEditor` has an editor per kind: a text field
(text, number, link, a native date picker for dates), a `Select` for a select, a listbox for a
multi-select and a member picker for a person, the last two in an `AnchoredPopover`; a
checkbox toggles without one. `ColumnMenu` edits (`ColumnDialog`: name, kind, choices, and the
count of cells a change clears from `POST /list-columns/{id}/preview` before it confirms),
moves, hides, deletes and adds columns, and marks a date column *Use as due date* (a *Due*
badge in its header). Rows drag by their grip; a row with comments shows their count where the
open button is, a board card under its fields. `ItemPanel` shows every field, hidden ones too,
with an always-open control per kind, and deletes the item. Under the fields `ItemComments`
holds the item's thread, rendered by `RichTextDisplay`, with the message composer
(`MessageInput`, a draft per item, mentions offered from the people who can see the list); the
author edits in place, the author or a channel moderator deletes after confirming.

`useLists.ts` holds the queries and mutations. A cell edit, a delete and a move are optimistic:
the move ranks the item locally with the same midpoint the server uses (`rankBetween`) and the
server's row replaces it. `listSync.ts` applies the `list.*` frames to the cache; an
`item_upserted` older than the cached row is ignored.

A `list` notification ("Ana assigned you to *Fix login*", "Ana mentioned you on *Fix login*")
carries `list_id` and `item_id`: the Activity row and the desktop notification open
`/lists/<list>?item=<item>`, and it rings even while its channel is on screen. A Later row for
an item due tomorrow shows the list's title and opens the item; ticking it off checks the item
on the server.

**API calls:** everything under `/workspaces/{id}/lists`, `/lists/{id}`, `/list-columns/{id}`,
`/list-items/{id}` (and its `/comments`), `/list-views/{id}`, `/list-comments/{id}`

---

### `workflows/WorkflowsPanel`, `WorkflowEditor`
*Workflows* in the workspace menu (not for guests) opens `WorkflowsPanel`, a right panel of
the workflows you built (an admin sees everyone's): name, trigger, on or off, the last run,
and who built it when that is somebody else. *New* opens `NewWorkflowDialog`, a blank
workflow or one of the drafts in `templates.ts` (welcome new members, daily standup reminder,
incident from a reaction, time-off request into a list). A row or the picked draft opens
`WorkflowEditor`, a
full-height `Modal` (`size="full"`): `TriggerCard` (schedule, webhook with its URL once saved
and its variables, a reaction in a channel with the emoji from `EmojiPicker`, someone
joining a channel, or a shortcut in a channel), then a `StepCard` per step (send a message to
a channel, a DM, or a form, to a person or, for a reaction, join or shortcut, to the person who
set it off), each movable and removable, with *Insert variable* putting `{{webhook.name}}`,
`{{trigger.user}}` or an earlier form's `{{form.name}}` at the caret. `FormStepEditor` edits a
form: its title and up to 10 questions, each a label, an answer kind, choices for a *Choice*,
and *Required*; a question's variable name follows its label until it is renamed. A message
step after a form can tick *Add the form answers below the message*. `ListStepEditor` edits
*Add an item to a list* (a list from `useWorkspaceLists`, then a field per column of
`useListDetail`) and *Update the item a step added* (the earlier step, then only its person,
select and checkbox columns): a select of choices, people or *Checked*/*Unchecked*, or a text
input, each offering only the variables that fit the column (`fits`), and *Fill from the form
answers* matches columns to questions by name. Moving or removing a step keeps an update step
pointing at the step that adds its item (`moveStep`, `removeStep`). *Create workflow* saves a
new one and it runs at once; *Save changes* keeps it on or off as it was; *Turn off* and *Turn
on* switch it. There are no drafts. *Run history* swaps the form for `RunHistory`, refetched every 10 seconds. Someone else's workflow,
or yours when only admins may build, opens read-only. `workflowModel.ts` holds the pure
helpers (schedule presets, insertion, reordering, who may build).

*Workspace settings → General* has a *Permissions* group: who can build workflows, use
`@channel`, `@here` and `@everyone`, and create public or private channels (every member or
admins only each). `lib/workspaceRules.ts` reads a rule from the settings; `lib/channelPermissions.ts`
mirrors the server's checks so the composer leaves the broadcasts out of its suggestions for
someone who would be refused, and *Create channel* offers only the kinds the person may make
(no *+* at all when neither). In a channel of more than 25 people the composer asks *This
notifies N people. Send anyway?* before a broadcast. A send the server refuses says why. A message whose
`metadata.bot.workflow_id` is set is tagged *Workflow* instead of *Bot*.

A message with `metadata.form` renders `FormCard` instead of its text: the title, and for its
recipient a *Fill in* button opening `FormDialog`, a `Modal` of shared fields (a text input,
text area, select of the choices or of the workspace's people, or a date input); everyone else
reads who it waits for, and once answered or expired it says so. A channel's composer shows
`ShortcutMenu`, a ⚡ button listing the channel's shortcuts, when it has any; picking one starts
it. `/workflow <name>` goes to the server like any other command.

**API calls:** `/workspaces/{id}/workflows`, `/workflows/{id}` (with `/enable`, `/disable`,
`/runs`, `/start`), `/channels/{id}/workflow-shortcuts`, `/workflow-runs/{id}/forms/{position}`

---

### `ThreadPanel`
Side panel showing thread replies for a parent message.

**Reads:** `useThreadMessages(parentId)`, `useUserCache`
**API calls:** GET `/messages/{id}/thread`, POST `/messages/{id}/thread`

A deleted root with no replies left takes none: the composer gives way to *This thread was
deleted* (`isClosedThread`). A card in *Threads* replies as the panel does, with *Also send to*
and scheduling.

---

### `SearchPanel`
Debounced search with **Messages | Files** tabs over one query and filter bar. On *Files*
the *Has* chip gives way to *Type* (`type:pdf` typed in the box does the same and switches to
*Files*). A file result (`features/search/FileHit`) shows the thumbnail or kind icon, the name
with the typed fragment marked, size, uploader, place (the hit's `channel_name`, or *Direct message*) and date, and for a match inside the file the server's snippet under the name, its matched words in `<mark>` (the snippet arrives as `{ text, hit }` parts and is rendered as text, never as HTML); an image opens the gallery,
anything else its message, and download is a hover action. The channel files panel's *Filter
by name* box asks the same route with `channel_id`.

**API calls:** GET `/search?q=...&limit=20`, GET `/search/files?q=...&limit=20`

`lib/fileKinds.ts` holds the icon and tone per server `kind` (`FILE_KINDS`) and `kindOf()`, the
same rules as the database's `file_kind_of()` for files that have no server kind yet (the
composer tray, message attachments). Both are tested against `fileKindCases.json`.

---

### `PinnedMessagesPanel`
Lists all pinned messages in the current channel.

**Reads:** `useChannelPins(channelId)`
**API calls:** GET `/channels/{id}/pins`

---

### `ChannelMembersPanel`
Shows members of the current channel with add/remove actions.

**Reads:** `useChannelMembers(channelId)`
**API calls:** POST `/channels/{id}/members`, DELETE `/channels/{id}/members/{userId}`

---

### `workspace/settings/WorkspaceSettingsModal`
Everything about the workspace in one `SectionedModal`, opened from the workspace menu's
*Workspace settings* (or *Invite people*, which opens *Members* with the invite form). An admin
sees *General*, *Members*, *User groups*, *Custom emoji*, *Apps*, *Integrations*, *Slack
import* and *Audit log*; a member sees the four they can use (`sections.ts`). *Slack import*
lists the last runs with their counts (guests, people who left Slack and archived channels
among them) and *Start again* under a failed run whose export is still stored
(`retryable`), which calls `POST /workspaces/{id}/slack-imports/{run}/retry`. Each section is
the former right panel's content (`MembersSection`, `IntegrationsSection`, …). `?settings=<section>`
in the address opens it at that section and is then removed (`useSettingsDialog`).

*General* saves as you change it, like *Preferences*: a name or description when the field is
left, the icon on upload (`POST /workspaces/{id}/icon`, which stores and sets it), the switches at once, each PATCH carrying only that field; a status
line says *Saving…*, *Saved* or why not. Deleting the workspace stays a danger zone at the
bottom. *Members* shows the invite form to admins only, since inviting is admin-only. After an
invite it shows the invite's link with a copy button, and says whether the email went, was not
sent because email is not set up, or could not be sent (`inviteMessage`). Above the members, `PendingInvites` lists the email invites still waiting to
be used, each with *Copy invite link* (GET `/workspaces/{id}/invites/{invite_id}/link`, then the clipboard) and
*Revoke* behind a confirmation. The invite page (`/invite/:token`) offers *Continue with SSO*
when the instance has it, starting `/api/auth/oidc/start?invite=<token>`, and so does the
sign-in page when it was reached from an invite.

**API calls:** PATCH `/workspaces/{id}`, DELETE `/workspaces/{id}` (optionally `?hard=true`),
POST `/workspaces/{id}/invites` `{ email, role }`, and each section's own.

---

### `profile/EditProfileModal` and `profile/PreferencesModal`
*Edit profile* is the person in the current workspace: display name, status (presets and
*Clear after*), bio and photo, saved with one button; the status is sent only when it changed,
so an untouched one keeps its expiry. An empty name, bio or photo shows the account's, as its
placeholder, and *Use account photo* drops the workspace's. *Preferences* is a vertical tab
list — *Notifications*, *Appearance*, *Account*, *Security* — over panels that save as they
change, except *Account*, the account's name, bio and photo, which has its own save button.
A photo picked under *Account* is uploaded to `POST /users/me/avatar` and set on the account at
once, so every workspace shows it; *Edit profile*'s photo stays a file of its workspace.
The *You* row reads the person from the users cache, so it shows the workspace profile. *Notifications* opens with `NotifyDefaultPanel`, the
workspace default level (*All new messages*, *Mentions*, *Nothing*), and ends with this device's
push, *Desktop notifications while the app is open* (`DesktopNotificationsPanel`, the
`notificationPrefs.desktopEnabled` switch, asking the browser first) and the workspace's
mention emails. An in-tab desktop notification is tagged with its channel, as the push is, so
one replaces the other instead of showing twice. It carries the instance whose socket sent it,
and a click locates the message through that instance's API, so a notification from a second
server opens its message there. Both open from the *You*
menu; the sidebar avatar opens *Edit profile*.

### `notifications/NotificationLevelMenu`
`NotificationLevelButton` is the bell in the channel header, in a DM's header and on a sidebar
row's hover. Its menu sets the caller's level for that channel — *Default (…)*, *All new
messages*, *Mentions*, *Nothing* — and, for a channel, *Mute channel*. It is a keyboard menu
(`menuitemradio`, arrows, Home/End). The bell shows the effective level: `BellRing` for all,
`BellOff` for nothing or muted; a sidebar row on nothing or muted carries a small `BellOff`.
On a phone the header's overflow menu opens the same menu.

### `notifications/ScheduleEditor` and `PauseMenuItems`
*Notification schedule* in *Preferences → Notifications*: a switch, day chips (with
*Weekdays* and *Every day*), and *From*/*To* selects in 30-minute steps, saved as they change;
*To* never offers the start time, and moving *From* onto the end pushes the end 30 minutes on.
The hours are in the profile's timezone, which the editor names. `PauseMenuItems` is the pause
section of the *You* menu and the Activity panel's moon: presets while notifications are on,
and "Paused by your schedule until 9:00" or "Paused until 14:30" with *Resume* while they are
not (`lib/pause.ts` decides which); it names the workspace it pauses (*Pause notifications in
Acme*, *Resume notifications in Acme*). The pause, the schedule and the email switch are the
current workspace's; *Preferences → Notifications* says which workspace that is. The profile
hover card shows a member's pause in that workspace from `notifications_paused_until` in the
member list.

### `notifications/KeywordsPanel`
*My keywords* in *Preferences → Notifications*: a field that adds a chip on Enter or a comma,
and a remove button per chip; every change saves the whole list. `lib/keywords.ts` refuses
what the server would (length, a leading `@` or `#`, a duplicate after folding, the limit of
50) before it is sent. The list is held in `useKeywordStore`, populated by
`useWorkspaceData`, so `MessageContent` marks a keyword with `<mark class="keyword-match">`
(the self-mention background, no pill) without a query per message. Matching folds case and
diacritics and needs a non-letter on both sides, as the server does.

A keyword notification does **not** move the sidebar's mention badge or unread count: those
counters are written with the message, and matching every member's keywords there would put a
regex scan on the send path. It shows up in Activity, as a push and as a desktop
notification. This is intended.

**API calls:** GET / PATCH `/workspaces/{id}/profile`, PUT / DELETE `/workspaces/{id}/profile/status`, GET / PATCH `/users/me` (*Account*), POST `/users/me/avatar` (account photo), POST `/files/upload/{workspaceId}` (workspace photo)

---

### `TypingIndicator`
Shows "X is typing..." text. Listens for `typing.indicator` (`{ channel_id, user_id, is_typing }`) on the global event bus and expires each entry after ~5s. No outbound data. (The local input sends `typing.start` / `typing.stop` over the WS; the server fans those out as `typing.indicator`.)

---

### `PresenceDot`
Small colored dot. Reads `usePresenceStore` for the given user ID. No outbound data.

---

## Zustand Stores

### `useWorkspaceStore`
UI-only state for the active workspace session. **No server data lives here** — messages, channels, and members are owned by React Query.

| State | Type | Description |
|-------|------|-------------|
| `currentWorkspace` | `Workspace \| null` | Selected workspace |
| `currentChannel` | `Channel \| null` | Selected channel |
| `currentUserRole` | `WorkspaceRole \| null` | User's role in the workspace |
| `currentUserId` | `string \| null` | Active user ID (for self-message checks) |
| `currentDmPartnerId` | `string \| null` | Active DM partner |
| `unreadChannels` | `Set<string>` | Channel IDs with unread messages |
| `mentionChannels` | `Set<string>` | Channel IDs with unread mentions |
| `mutedChannels` | `Set<string>` | Muted channel IDs (suppress unread/notify) |
| `unreadDmPartners` | `Set<string>` | DM partner IDs with unread messages |
| `activeHuddleChannels` | `Map<string, {...}>` | Channels with a live huddle |

Key methods:

| Method | What it does |
|--------|--------------|
| `selectWorkspace(ws)` | Sets current workspace, WS `subscribe` to workspace, sets active instance |
| `selectChannel(ch)` | Sets current channel + WS `channel.join` |
| `selectDmPartner(id)` | Sets current DM partner (clears current channel) |
| `setCurrentUserRole(role)` | Updates role in store |
| `markChannelRead(chId)` | Removes channel from unread/mention sets |
| `setChannelMuted` / `hydrate*` | Mute toggling and bulk hydration of unread/muted/DM state |

---

### `useInstanceStore`
Manages all connected instances and the active one. `{ url, wsUrl?, user }` per instance is
persisted to `localStorage` (`chat_instances`). For cross-origin instances, tokens are kept in a
separate `chat_tokens` map; same-origin uses cookies
storage (see Authentication Model).

| State | Description |
|-------|-------------|
| `instances` | Array of `{ url, wsUrl?, user }` |
| `activeInstanceUrl` | Currently active instance |
| `hydrated` | Whether `restoreInstances` has finished (gates routing) |

Key methods:

| Method | Description |
|--------|-------------|
| `addInstance(url, email, pass, wsUrl?)` | POST `/auth/login`, wires `ApiClient`/WS, persists tokens per origin, saves `{ url, wsUrl?, user }`, connects WS |
| `addValidatedInstance(config)` | Adds an already-authenticated instance (used after complete-registration) |
| `removeInstance(url)` | POST `/auth/logout`, disconnect WS, clear stored tokens, remove from store + localStorage |
| `setActiveInstance(url)` | Switch active instance |
| `updateInstanceUser(url, user)` | Replace the cached user for an instance |
| `restoreInstances()` | Re-hydrate instances; refresh the session (same-origin GET `/users/me`, cross-origin token refresh); a failed refresh removes the stale instance, then reconnect WS |

---

### `usePresenceStore`
Maps `userId → 'online' \| 'away' \| 'offline'`. Updated by incoming WebSocket `presence.changed` / `presence.batch` events, which include the signed-in person's own status. *You → Set yourself away / active* (`useSetAway`, `PUT /users/me/presence`) sets it at once and puts it back if the request fails.

---

### `useUserCache`
In-memory map of `userId → { displayName, avatarUrl }`. Populated when workspace members load. Read by `MessageItem`, `ThreadPanel`, `ChannelSidebar`.

---

## React Query Hooks

All hooks accept an optional `instanceUrl` to target a specific connected instance.

### Auth (`features/instance/hooks/useAuth.ts`)
| Hook | Call | Description |
|------|------|-------------|
| `useCurrentUser()` | — | Returns the active instance's user from `useInstanceStore` (no API call) |
| `useAddInstance()` | POST `/auth/login` | Login mutation; response `{ user, expires_in, access_token, refresh_token }`; tokens persisted per origin |
| `useCompleteRegistration()` | POST `/auth/complete-registration` | Same flow as login; adds a validated instance |
| `useLogout(instanceUrl?)` | POST `/auth/logout` | Removes the instance (or all), clears query cache |

Token refresh is **not** a hook — `ApiClient` refreshes automatically on 401 / near-expiry via `POST /auth/refresh` (cookie same-origin, bearer cross-origin).

### Workspaces
| Hook | Call | Description |
|------|------|-------------|
| `useWorkspaces()` | GET `/workspaces` (all instances, parallel) | All workspaces across all instances |
| `useWorkspace(wsId)` | GET `/workspaces/{id}` | Single workspace |
| `useWorkspaceMembers(wsId, instanceUrl)` | GET `/workspaces/{id}/members` | Workspace members |
| `useWorkspaceChannels(wsId, instanceUrl)` | GET `/workspaces/{id}/channels` | Channels list |
| `useDeletedWorkspaces()` | GET `/workspaces/deleted` | Soft-deleted workspaces |
| `useRestoreWorkspace()` | POST `/workspaces/{id}/restore` | Restore mutation |
| `useCreateWorkspace()` | POST `/workspaces` | Create workspace mutation |
| `useCreateChannel()` | POST `/workspaces/{id}/channels` | Create channel mutation |

### Messages
| Hook | Call | Description |
|------|------|-------------|
| `useMessages(channelId)` | GET `/channels/{id}/messages?limit=50&cursor=...` | Infinite query; `select` reverses each page to newest-last |
| `useSendMessage(channelId, userId)` | POST `/channels/{id}/messages` `{ content, client_message_id }` | Optimistic insert, idempotent retry; confirmed by WS `message.new` |
| `useEditMessage()` | PATCH `/messages/{id}` | Optimistic edit, rollback on error |
| `useDeleteMessage()` | DELETE `/messages/{id}` | Soft-delete on success (`deleted_at`) |
| `useReactToMessage()` | POST `/messages/{id}/reactions` | Optimistic add reaction |
| `useRemoveReaction()` | DELETE `/messages/{id}/reactions/{emoji}` | Optimistic remove reaction |
| `usePinMessage()` | POST/DELETE `/messages/{id}/pin` | Optimistic pin/unpin; invalidates pins on settle |

### Threads
| Hook | Call | Description |
|------|------|-------------|
| `useThreadMessages(parentId)` | GET `/messages/{id}/thread` | Thread replies |
| `useSendThreadReply(parentId, chId)` | POST `/messages/{id}/thread` | Reply mutation |

### Channels (`features/channel/hooks/useChannels.ts`)
| Hook | Call | Description |
|------|------|-------------|
| `useChannelMembers(channelId)` | GET `/channels/{id}/members` | Channel members |
| `useChannelPins(channelId)` | GET `/channels/{id}/pins` | Pinned messages |
| `useUnreadChannels(wsId, instanceUrl?)` | GET `/workspaces/{id}/channels/unread` | Unread and mention counts per channel, conversations included |
| `useBrowsableChannels` / `useJoinChannel` / `useLeaveChannel` | GET `/workspaces/{id}/channels/browse`, POST `/channels/{id}/join`, DELETE `/channels/{id}/members/{userId}` | Channel browser |
| `useSetChannelMuted(wsId, instanceUrl?)` | PATCH `/channels/{id}/notifications` `{ muted }` | Mute/unmute (optimistic) |

(`useChannel` lives in this module's barrel as needed; the channel object itself is also carried in the workspace channels list.)

### Conversations (`features/messaging/hooks/useConversations.ts`)
| Hook | Call | Description |
|------|------|-------------|
| `useConversations(wsId, instanceUrl?)` | GET `/workspaces/{id}/conversations` | Direct and group conversations, newest message first, with participants and the caller's `last_read_at` |
| `useOpenConversation(wsId, instanceUrl?)` | POST `/workspaces/{id}/conversations` `{ participant_ids }` | Finds or creates one; the result's id is a channel id |
| `useMarkConversationRead(wsId, instanceUrl?)` | POST `/channels/{id}/read` | Reads the conversation up to its newest loaded message, as a channel |

Messages, threads, reactions, edits, pins, saved items and scheduled sends in a conversation
use the channel hooks above with the conversation id as `channelId`.

### Notifications (`features/notifications/hooks/useNotifications.ts`)
| Hook | Call | Description |
|------|------|-------------|
| `useNotifications(wsId)` | GET `/workspaces/{id}/notifications?limit=50` | Notifications list (normalized) |
| `useUnreadNotificationCount(wsId)` | GET `/workspaces/{id}/notifications/unread-count` | Unread count |
| `useWorkspaceUnreadCounts(workspaces)` | GET `/workspaces/{id}/notifications/unread-count` (parallel) | Per-workspace counts for the sidebar |
| `useMarkNotificationsRead(wsId)` | POST `/notifications/read` `{ notification_ids }` | Mark specific read (optimistic) |
| `useMarkChannelNotificationsRead(wsId)` | POST `/workspaces/{id}/channels/{channelId}/notifications/read` | Mark a channel's notifications read |
| `useMarkAllNotificationsRead(wsId)` | POST `/workspaces/{id}/notifications/read-all` | Mark all read |
| `useDndStatus()` / `useSetDnd()` | GET/PATCH `/workspaces/{id}/notifications/dnd` (current workspace) | `{ dnd_until, schedule_paused_until }`; setting `null` resumes, including during a scheduled pause |
| `useNotifyDefault(wsId)` / `useSetNotifyDefault(wsId)` | GET/PUT `/workspaces/{id}/notify-default` | The caller's default level (optimistic); a change refetches channels and conversations |
| `useSetNotifyLevel(wsId)` | PUT `/channels/{id}/notify-level` `{ level }` | One channel's or DM's level; the answer is merged into the channels and conversations caches |
| `useNotificationSchedule()` / `useSetNotificationSchedule()` | GET/PUT `/workspaces/{id}/notifications/schedule` (current workspace) | The caller's schedule (optimistic); a change refetches the DND status |
| `useNotificationKeywords(wsId)` / `useSetNotificationKeywords(wsId)` | GET/PUT `/workspaces/{id}/notification-keywords` | The caller's keywords; a change is optimistic and rolls back on error |

### Huddles (`features/huddle/hooks/useHuddle.ts`)
`useActiveHuddles(wsId, instanceUrl?)` backfills currently-active huddles for the workspace.

`HuddleController` carries the media over a `HuddleTransport` (`features/huddle/lib`) and
sees nothing else of it: set a microphone, camera or screen track, mute, add or drop a peer,
start over after a reconnect, close. `MeshManager` is the peer-to-peer one and does its own
offer, answer and ICE signalling over the gateway. `SfuTransport` joins the huddle's LiveKit
room with a token from `POST /huddles/{id}/sfu-token`, through the same ICE servers, and is
loaded only when it is used. Mute, camera, screen share, raised hand and reactions still go
over the gateway on both, so nothing else cares which one carries the media. Who is speaking
comes from `useSpeaking` on the mesh and from LiveKit on the SFU (`useHuddleStore.transport`
says which).

Every huddle starts on the mesh. When the gateway moves it (`huddle.transport`, or `transport:
"sfu"` in the `huddle.members` snapshot a late joiner gets), the controller swaps in a
`Promotion`: it opens the SFU with `openSfu`, publishes the same microphone, camera, screen and
mute there, and sends every change to both while it waits. Tracks arriving from LiveKit are
held back, and the mesh keeps playing, until it has the audio of everybody it was meshed with,
or eight seconds pass for someone who never arrives; then it closes the mesh and plays the
held tracks in the same moment. Nobody new is meshed with once the move has started. A browser
that cannot reach the SFU keeps its mesh with whoever is still on it and says so.

---

## Infrastructure (`src/lib/`)

### `api.ts` — `ApiClient`
An `openapi-fetch` client typed by `src/api/schema.d.ts`. All requests include
`credentials: 'include'`. Cross-origin/desktop clients additionally set
`Authorization: Bearer <access>` from the in-memory token; same-origin relies on the cookie.
- Same-origin: routes through `/api` (Vite proxy)
- Cross-origin: routes to `{instanceUrl}/api`
- `typed((c) => c.GET('/channels/{ch_id}/messages', { params }))` — the only way to call a JSON route; paths, params, bodies and responses come from the schema
- `upload<T>(path, formData)` — the one multipart path (files, avatars, Slack archives)
- Regenerate with `npm run api:types` after `cargo run --bin chat-openapi > src/api/openapi.json`
- On 401: single-flight refresh once, then retry via `POST /auth/refresh`; if refresh fails, fires `onSessionExpired`
- `onTokensChanged` lets the store persist rotated tokens (cross-origin `chat_tokens`)
- Singleton `api` for default instance; `instanceManager.get(url).api` for others

### `ws.ts` — `WebSocketClient`
Reconnecting WebSocket wrapper. Reconnect uses exponential backoff with jitter (base 1s, factor 2,
cap 30s).
- `connect()` — resolves a valid token via `getToken()` and opens the socket with `['bearer', token]` as the subprotocol (no subprotocol if no token, falling back to the cookie)
- On (re)open it replays state: `subscribe` for the last workspace and `channel.join` for every joined channel, and `activity` when the person is active (a new socket starts idle)
- `lib/activity.ts` decides active: the page is visible and has had input (pointer, key, wheel, touch) in the last two minutes; hiding the tab or leaving the page (`pagehide`) is idle at once. Every client reports each change as `{ type: 'activity', active }`, and the server holds push only while a session is active
- `send(event)` — sends JSON (only when the socket is OPEN)
- `subscribe(wsId)`, `joinChannel(chId)`, `leaveChannel(chId)` — send `subscribe` / `channel.join` / `channel.leave` and remember the desired state for replay
- `addReconnectListener(fn)` — fires after a *reconnect* (used to wire `backfillAfterReconnect`)
- Waking up does not wait for the backoff: `online`, and the tab becoming visible, reconnect at once when the socket is closed. A socket that still looks open is sent a `ping`; if nothing arrives within two seconds (a laptop's half-open socket after sleep) it is replaced. `disconnect()` and a revoked session stop all of this
- `lib/realtimeBackfill.ts`: after every reconnect `backfillAfterReconnect` refetches the open views (messages, Activity, conversations, active huddles) and leaves counters to the replayed events, since a refetched unread count already includes a message the replay then bumps again. On `sync.refetch_required`, when there is no replay, `refetchAfterGap` also invalidates threads and their state, followed threads, every `['channels', …]` key (pins, unread counts, members), `unreads`, the sidebar and *Later*
- `on(type, handler)` — subscribe to event type (returns unsub fn)
- All received events are forwarded to `globalEventBus` with the client's instance URL, which a handler gets as its second argument
- Singleton `wsClient` for default; `instanceManager.get(url).ws` for others

Message types the client **sends**: `subscribe`, `channel.join`, `channel.leave`, `typing.start`, `typing.stop`, `activity`.

### New versions — `lib/appVersion.ts`, `lib/chunkReload.ts`
A build writes `version.json` (`{ version }`, from `APP_VERSION` or the build time) and
compiles the same id in as `import.meta.env.VITE_BUILD_ID`. nginx serves `index.html` and
`version.json` with `Cache-Control: no-cache`; the hashed assets stay immutable.
- `watchForNewBuild(running, onNew)` fetches `/version.json` when the tab comes back, when the network does, and every ten minutes; a different version shows `NewVersionBanner` (*A new version is available*, *Reload*) at the top of the workspace page. A dev build (`dev`) does not watch
- `reloadWhenChunksAreGone()` (in `main.tsx`) reloads once on `vite:preloadError`, the error a lazy page or the emoji picker raises when its chunk is gone; a second failure within a minute is left to surface instead of looping, and so is one where `sessionStorage` cannot remember the reload
- The release image copies the previous release's chunks in beside its own (`PREVIOUS_IMAGE` build argument, the list in `release-assets.txt`), so a tab opened before a deploy keeps loading for one release

### `instances.ts` — `InstanceManager`
Registry of `{ api: ApiClient, ws: WebSocketClient }` per instance URL.
- `get(url)` — get or create client pair
- `remove(url)` — disconnect WS + remove from registry
- Used by all hooks/stores that need instance-aware requests

### `globalEventBus.ts` — `GlobalEventBus`
Typed pub/sub bus over the `ServerFrame` union generated into `src/api/serverFrames.ts`
(`cargo run --bin chat-ts-events`; `lib/serverEvents.ts` re-exports it). All WebSocket clients
from all instances publish here.
- `on(type, handler)` — subscribe, narrowed to the event variant (returns unsub fn)
- `emit(event)` — broadcast

Event types include: `workspace.created/updated/deleted/restored`, `member.added/removed`,
`channel.created/updated/member_added/member_removed`, `message.new/updated/deleted/pinned`,
`reaction.added/removed`, `conversation.created`, `notification`,
`presence.changed`, `presence.batch`, `typing.indicator`, and the `huddle.*` signaling/lifecycle
events.

### `wsQuerySync.ts` — `useWebSocketQuerySync()`
Called once in `App.tsx`. Listens on `globalEventBus` and updates React Query cache. (Presence,
typing, and `notification` events are handled separately — by `usePresenceStore`,
`TypingIndicator`, and `NotificationStream`/DM handlers respectively — not here.)

| Event | Cache action |
|-------|-------------|
| `message.new` | Upsert into message list (confirm optimistic / append); if thread reply, append to thread cache + increment `reply_count` on parent; for a channel, mark unread unless current/muted; for a conversation, reorder the conversation list and mark it unread if incoming and not open; sound and desktop notifications come from the `notification` frame instead |
| `message.updated` | Patch the cached message (content, `updated_at`, `metadata` — a poll's counts) in the channel list and in the thread it belongs to |
| `message.deleted` | Soft-delete: set `deleted_at` on cached message |
| `message.pinned` | Invalidate pins query + patch `is_pinned` on cached message |
| `reaction.added` / `reaction.removed` | Patch reactions array on cached message |
| `channel.created` | Invalidate workspace channels list |
| `channel.updated` | Invalidate single channel query |
| `channel.member_added` / `channel.member_removed` | Invalidate channel members query |
| `member.added` / `member.removed` | Invalidate workspace members query |
| `workspace.created` / `workspace.restored` | Invalidate workspaces list |
| `workspace.deleted` | Invalidate workspaces + deleted list; navigate away unless workspace/instance admin |
| `workspace.emoji_changed` | Invalidate that workspace's custom emoji, so `:name:` renders and the picker offers it without a reload |
| `workspace.updated` | Invalidate single workspace + workspaces list |
| `conversation.created` | Invalidate the conversation list and join the new channel on the socket |
| `huddle.started` / `huddle.ended` | Set/clear the channel's active-huddle entry in `useWorkspaceStore` |
| `list.upserted` / `list.deleted` | Patch or refetch the list, refetch the lists menus (`features/lists/listSync.ts`) |
| `list.columns_changed` | Replace the columns; refetch the items when `items_rewritten` |
| `list.views_changed` | Replace the views |
| `list.item_upserted` / `list.item_deleted` | Place the row by rank, or drop it |
| `list.comment_upserted` / `list.comment_deleted` | Place or drop the comment in an open item's thread |

---

## Event Flow: Sending a Message

```
1. User types → MessageInput.onTyping()
   → wsClient.send({ type: 'typing.start', channel_id })

2. User hits Enter → WorkspacePage.handleSend()
   → useSendMessage.mutate({ content, id })
     → optimistic: add pending message to React Query cache
     → POST /channels/{id}/messages { content, id }

3. API persists to DB → publishes to Redis

4. chat-realtime receives from Redis → pushes WS event

5. WebSocketClient receives { type: 'message.new', message }
   → globalEventBus.emit(event)

6. wsQuerySync hears 'message.new' → confirms optimistic message in React Query cache (sets pending: false)

7. MessageList re-renders with confirmed message (pending flag removed)
```

---

## State Ownership Summary

| State | Tool | Populated by |
|-------|------|-------------|
| Current workspace / channel | Zustand | User click |
| Messages list | React Query | API + WS events |
| Unread / mention markers | Zustand | WS `channel.read_state` and `notification` (`kind: mention`) events |
| User presence | Zustand | WS presence events |
| Workspace list | React Query | API + WS workspace events |
| Workspace members | React Query | API + WS member events |
| Channel members | React Query | API + WS channel events |
| Pinned messages | React Query | API + WS pin events |
| Thread messages | React Query | API |
| Conversations | React Query | API + WS `conversation.created`; their messages are the message list keyed by the conversation id |
| Instance config (url + wsUrl? + user) | Zustand + localStorage (`chat_instances`) | Login response `{ user }` |
| Auth tokens | Access in memory (`ApiClient`); refresh in cookie (same-origin) / `chat_tokens` localStorage (cross-origin) | `/auth/login`, `/auth/refresh` |
| Current user | Zustand (from instance) | Login response |
| User display cache | Zustand | Populated from members list |
