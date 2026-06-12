# Frontend

A React + Vite single-page app. State is split between **TanStack Query** (server state)
and **Zustand** (UI state); WebSocket events flow through a typed **global event bus** that
reconciles them into the Query cache.

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
- **A typed event bus bridges WebSocket → cache.** `serverEvents.ts` defines a discriminated
  union of server events; `wsQuerySync` is the single reducer that turns each event into the
  right cache mutation (upsert/patch/remove), so realtime updates and HTTP responses converge
  on one source of truth.

### Structure

- **Feature-modular** under `src/features/*` (workspace, channel, messaging, navigation,
  notifications, huddle), each with a barrel. Smart logic lives in hooks; views stay thin — e.g.
  `useWorkspaceController` holds the page's data + handlers and `WorkspacePage` just renders.
- **`hooks/queries/*`** wrap every endpoint with Query/Mutation hooks and optimistic updates.
- **`lib/`** is the infrastructure layer: `api` (fetch client with single-flight 401 refresh),
  `ws` (reconnecting WebSocket with backoff + re-subscribe + reconnect backfill), `instances`
  (multi-instance manager), `messageCache` (cache helpers), `electron` (desktop bridge),
  `wsQuerySync`/`globalEventBus`/`serverEvents` (typed WS → cache reducer).

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
- **Desktop** — the same SPA runs in an Electron shell (`electron/`) that adds native
  notifications, a dock badge, `chatsystems://` deep links via a context-isolated preload, and
  encrypted at-rest refresh-token storage (`safeStorage`). The renderer uses `HashRouter` in
  Electron (`BrowserRouter` on the web).

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
| Electron desktop | in memory | encrypted on disk via `safeStorage` (main process `auth.json`) | `Authorization: Bearer` header + WS `bearer` subprotocol |

**Consequences for the frontend:**
- All `fetch` calls use `credentials: 'include'`; cross-origin/desktop also attach `Authorization: Bearer <access>`
- `WebSocketClient` resolves a valid token (`api.getValidToken()`) and opens the socket with `['bearer', token]` as the WS subprotocol; if no token is available it connects without one (relying on the cookie)
- On 401, `ApiClient` refreshes once (single-flight) then retries; same-origin refresh is a cookie-only `POST /auth/refresh`, cross-origin sends the stored refresh token as a bearer header, and Electron delegates refresh to the main process (`auth:refresh`). If refresh fails, `onSessionExpired` removes the instance.
- The backend requires `CORS_ORIGINS` (comma-separated allowed origins) and validates the WS upgrade `Origin` against it; wildcard `*` is not allowed when credentials are enabled
- `localStorage` (`chat_instances`) stores only `{ url, wsUrl?, user }` per instance — no tokens for same-origin/Electron; cross-origin tokens live in the separate `chat_tokens` map

---

## Pages

### `WorkspacePage`
Main app container, kept thin: it calls `useWorkspaceController()` for all data and
handlers and renders the layout (sidebars, conversation, right panels). The logic lives
in the hook, not the view. It is mounted by several routes that all resolve to the same
page: `/app/:workspaceId?`, `/app/:workspaceId/:channelId`,
`/app/:workspaceId/:channelId/:messageId`, and `/app/:workspaceId/dm/:dmUserId` (DM view).
When a `dmUserId` param is present the conversation area renders `DmView` instead of the
channel message list.

- **`features/workspace/hooks/useWorkspaceController`** — the "smart" hook: wires every
  query/store, the URL↔store sync effects, presence/notification init, WebSocket join +
  typing, and the send/upload/navigate handlers. Returns one object the view consumes.
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
**Reads:** GET `/admin/users?limit=200`
**Writes:**
- POST `/admin/users/{id}/suspend` / `/admin/users/{id}/activate` — toggle account status
- PATCH `/admin/users/{id}/instance-role` `{ is_instance_admin }` — grant/revoke instance admin

(Workspace deletion lives in `SettingsPanel`, not here.)

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
Top bar with channel name, topic, and buttons for search / pins / members.

**Reads:** current channel
**Emits:** `onToggleSearch()`, `onTogglePins()`, `onToggleChannelMembers()`

---

### `messaging/MessageList`
Infinite-scrolling message list (`flex-col-reverse` so newest messages appear at the bottom). Scrolling to top triggers `fetchNextPage()`.

**Reads:** `useMessages(channelId)` — infinite query, cursor-based pagination, 50 messages per page
**Emits:** `onThreadOpen(message)` when user clicks reply icon

---

### `messaging/MessageInput`
Text input with file attachment button and send button.

**Reads:** channel name
**Emits:** `onSend(content)`, `onFileUpload(file)`, `onTyping()` on each keystroke

---

## Components

### `MessageItem`
Single message row with reactions, edit/delete, thread reply, and pin actions. It does not
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

---

### `ThreadPanel`
Side panel showing thread replies for a parent message.

**Reads:** `useThreadMessages(parentId)`, `useUserCache`
**API calls:** GET `/messages/{id}/thread`, POST `/messages/{id}/thread`

---

### `SearchPanel`
Debounced full-text search across messages.

**API calls:** GET `/search?q=...&limit=20`

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

### `MembersPanel`
Workspace members list with an invite-by-email form (role picker: member / admin / guest).

**Reads:** `useWorkspaceMembers(workspaceId, instanceUrl)`
**API calls:** POST `/workspaces/{id}/invites` `{ email, role }` — server either adds an existing user directly or sends an invite

---

### `SettingsPanel`
Workspace name, description, and deletion (soft-delete by default; `?hard=true` for hard delete).

**API calls:** PATCH `/workspaces/{id}`, DELETE `/workspaces/{id}` (optionally `?hard=true`)

---

### `UserProfilePanel`
Edit display name, bio, and avatar (URL or upload).

**API calls:** GET `/users/me`, PATCH `/users/me` `{ display_name, bio, avatar_url }`, POST `/files/upload/avatars` (avatar upload)

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
separate `chat_tokens` map; same-origin uses cookies and Electron uses encrypted main-process
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
| `restoreInstances()` | Re-hydrate instances; refresh the session (same-origin GET `/users/me`, cross-origin/Electron token refresh); a failed refresh removes the stale instance, then reconnect WS |

---

### `usePresenceStore`
Maps `userId → 'online' \| 'away' \| 'offline'`. Updated by incoming WebSocket `presence.changed` / `presence.batch` events.

---

### `useUserCache`
In-memory map of `userId → { displayName, avatarUrl }`. Populated when workspace members load. Read by `MessageItem`, `ThreadPanel`, `ChannelSidebar`.

---

## React Query Hooks

All hooks accept an optional `instanceUrl` to target a specific connected instance.

### Auth (`hooks/queries/useAuth.ts`)
| Hook | Call | Description |
|------|------|-------------|
| `useCurrentUser()` | — | Returns the active instance's user from `useInstanceStore` (no API call) |
| `useAddInstance()` | POST `/auth/login` | Login mutation; response `{ user, expires_in, access_token, refresh_token }`; tokens persisted per origin |
| `useCompleteRegistration()` | POST `/auth/complete-registration` | Same flow as login; adds a validated instance |
| `useLogout(instanceUrl?)` | POST `/auth/logout` | Removes the instance (or all), clears query cache |

Token refresh is **not** a hook — `ApiClient` refreshes automatically on 401 / near-expiry via `POST /auth/refresh` (cookie same-origin, bearer cross-origin, IPC in Electron).

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
| `useSendMessage(channelId, userId)` | POST `/channels/{id}/messages` `{ content, id }` | Optimistic insert; confirmed by WS `message.new` |
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

### Channels (`hooks/queries/useChannels.ts`)
| Hook | Call | Description |
|------|------|-------------|
| `useChannelMembers(channelId)` | GET `/channels/{id}/members` | Channel members |
| `useChannelPins(channelId)` | GET `/channels/{id}/pins` | Pinned messages |
| `useUnreadChannelIds(wsId, instanceUrl?)` | GET `/workspaces/{id}/channels/unread` | Unread channel IDs |
| `useSetChannelMuted(wsId, instanceUrl?)` | PATCH `/channels/{id}/notifications` `{ muted }` | Mute/unmute (optimistic) |

(`useChannel` lives in this module's barrel as needed; the channel object itself is also carried in the workspace channels list.)

### Direct Messages (`hooks/queries/useDm.ts`)
| Hook | Call | Description |
|------|------|-------------|
| `useDmConversations(wsId, instanceUrl?)` | GET `/workspaces/{id}/dm` | DM conversation list (sorted by last message) |
| `useDirectMessages(wsId, partnerId, instanceUrl?)` | GET `/workspaces/{id}/dm/{partnerId}?limit=50&before=...` | Infinite DM thread |
| `useSendDirectMessage(...)` | POST `/workspaces/{id}/dm/{partnerId}` `{ content, id }` | Optimistic send |
| `useEditDirectMessage(...)` / `useDeleteDirectMessage(...)` | PATCH/DELETE `/workspaces/{id}/dm/{partnerId}/{messageId}` | Optimistic edit/soft-delete |
| `useReactToDm(...)` / `useRemoveDmReaction(...)` | POST/DELETE `/workspaces/{id}/dm/{partnerId}/{messageId}/reactions[/{emoji}]` | Optimistic DM reactions |
| `useMarkDmRead(wsId, instanceUrl?)` | POST `/workspaces/{id}/dm/{partnerId}/read` | Mark DM read |

### Notifications (`hooks/queries/useNotifications.ts`)
| Hook | Call | Description |
|------|------|-------------|
| `useNotifications(wsId)` | GET `/workspaces/{id}/notifications?limit=50` | Notifications list (normalized) |
| `useUnreadNotificationCount(wsId)` | GET `/workspaces/{id}/notifications/unread-count` | Unread count |
| `useWorkspaceUnreadCounts(workspaces)` | GET `/workspaces/{id}/notifications/unread-count` (parallel) | Per-workspace counts for the sidebar |
| `useMarkNotificationsRead(wsId)` | POST `/notifications/read` `{ notification_ids }` | Mark specific read (optimistic) |
| `useMarkChannelNotificationsRead(wsId)` | POST `/workspaces/{id}/channels/{channelId}/notifications/read` | Mark a channel's notifications read |
| `useMarkAllNotificationsRead(wsId)` | POST `/workspaces/{id}/notifications/read-all` | Mark all read |
| `useDndStatus()` / `useSetDnd()` | GET/PATCH `/notifications/dnd` | Do-not-disturb window |

### Huddles (`hooks/queries/useHuddle.ts`)
`useActiveHuddles(wsId, instanceUrl?)` backfills currently-active huddles for the workspace.

---

## Infrastructure (`src/lib/`)

### `api.ts` — `ApiClient`
Thin fetch wrapper. All requests include `credentials: 'include'`. Cross-origin/desktop clients
additionally set `Authorization: Bearer <access>` from the in-memory token; same-origin relies on
the cookie. The client decodes the access token's `exp` and refreshes ~10s before expiry.
- Same-origin: routes through `/api` (Vite proxy)
- Cross-origin: routes to `{instanceUrl}/api`
- Methods: `get<T>`, `post<T>`, `patch<T>`, `delete<T>`, `upload<T>` (multipart)
- On 401: single-flight refresh once, then retry; refresh delegates to `refreshHandler` in Electron, otherwise `POST /auth/refresh`; if refresh fails, fires `onSessionExpired`
- `onTokensChanged` lets the store persist rotated tokens (cross-origin `chat_tokens`)
- Singleton `api` for default instance; `instanceManager.get(url).api` for others

### `ws.ts` — `WebSocketClient`
Reconnecting WebSocket wrapper. Reconnect uses exponential backoff with jitter (base 1s, factor 2,
cap 30s).
- `connect()` — resolves a valid token via `getToken()` and opens the socket with `['bearer', token]` as the subprotocol (no subprotocol if no token, falling back to the cookie)
- On (re)open it replays state: `subscribe` for the last workspace and `channel.join` for every joined channel
- `send(event)` — sends JSON (only when the socket is OPEN)
- `subscribe(wsId)`, `joinChannel(chId)`, `leaveChannel(chId)` — send `subscribe` / `channel.join` / `channel.leave` and remember the desired state for replay
- `addReconnectListener(fn)` — fires after a *reconnect* (used to wire `backfillAfterReconnect`)
- `on(type, handler)` — subscribe to event type (returns unsub fn)
- All received events are forwarded to `globalEventBus`
- Singleton `wsClient` for default; `instanceManager.get(url).ws` for others

Message types the client **sends**: `subscribe`, `channel.join`, `channel.leave`, `typing.start`, `typing.stop`.

### `instances.ts` — `InstanceManager`
Registry of `{ api: ApiClient, ws: WebSocketClient }` per instance URL.
- `get(url)` — get or create client pair
- `remove(url)` — disconnect WS + remove from registry
- Used by all hooks/stores that need instance-aware requests

### `globalEventBus.ts` — `GlobalEventBus`
Typed pub/sub bus (events are the `AppServerEvent` discriminated union in `serverEvents.ts`). All
WebSocket clients from all instances publish here.
- `on(type, handler)` — subscribe, narrowed to the event variant (returns unsub fn)
- `emit(event)` — broadcast

Event types include: `workspace.created/updated/deleted/restored`, `member.added/removed`,
`channel.created/updated/member_added/member_removed`, `message.new/updated/deleted/pinned`,
`reaction.added/removed`, `dm.new/updated/deleted`, `dm.reaction.added/removed`, `notification`,
`presence.changed`, `presence.batch`, `typing.indicator`, and the `huddle.*` signaling/lifecycle
events.

### `wsQuerySync.ts` — `useWebSocketQuerySync()`
Called once in `App.tsx`. Listens on `globalEventBus` and updates React Query cache. (Presence,
typing, and `notification` events are handled separately — by `usePresenceStore`,
`TypingIndicator`, and `NotificationStream`/DM handlers respectively — not here.)

| Event | Cache action |
|-------|-------------|
| `message.new` | Upsert into message list (confirm optimistic / append); if thread reply, append to thread cache + increment `reply_count` on parent; mark channel unread unless current/muted |
| `message.updated` | Patch content/updated_at in cached message |
| `message.deleted` | Soft-delete: set `deleted_at` on cached message |
| `message.pinned` | Invalidate pins query + patch `is_pinned` on cached message |
| `reaction.added` / `reaction.removed` | Patch reactions array on cached message |
| `channel.created` | Invalidate workspace channels list |
| `channel.updated` | Invalidate single channel query |
| `channel.member_added` / `channel.member_removed` | Invalidate channel members query |
| `member.added` / `member.removed` | Invalidate workspace members query |
| `workspace.created` / `workspace.restored` | Invalidate workspaces list |
| `workspace.deleted` | Invalidate workspaces + deleted list; navigate away unless workspace/instance admin |
| `workspace.updated` | Invalidate single workspace + workspaces list |
| `dm.new` | Upsert into DM thread + conversation list; mark unread / notify if incoming and not active |
| `dm.updated` / `dm.deleted` | Patch / soft-delete cached DM |
| `dm.reaction.added` / `dm.reaction.removed` | Patch reactions on cached DM |
| `huddle.started` / `huddle.ended` | Set/clear the channel's active-huddle entry in `useWorkspaceStore` |

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
| Unread / mention markers | Zustand | WS notification events |
| User presence | Zustand | WS presence events |
| Workspace list | React Query | API + WS workspace events |
| Workspace members | React Query | API + WS member events |
| Channel members | React Query | API + WS channel events |
| Pinned messages | React Query | API + WS pin events |
| Thread messages | React Query | API |
| DM conversations / messages | React Query | API + WS `dm.*` events |
| Instance config (url + wsUrl? + user) | Zustand + localStorage (`chat_instances`) | Login response `{ user }` |
| Auth tokens | Access in memory (`ApiClient`); refresh in cookie (same-origin) / `chat_tokens` localStorage (cross-origin) / encrypted disk (Electron) | `/auth/login`, `/auth/refresh` |
| Current user | Zustand (from instance) | Login response |
| User display cache | Zustand | Populated from members list |
