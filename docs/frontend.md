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
  notifications), each with a barrel. Smart logic lives in hooks; views stay thin — e.g.
  `useWorkspaceController` holds the page's data + handlers and `WorkspacePage` just renders.
- **`hooks/queries/*`** wrap every endpoint with Query/Mutation hooks and optimistic updates.
- **`lib/`** is the infrastructure layer: `api` (fetch client with single-flight 401 refresh),
  `ws` (reconnecting WebSocket with backoff + re-subscribe + backfill), `instances`
  (multi-instance manager), `messageCache` (cache helpers), `electron` (desktop bridge).

### Cross-cutting decisions

- **Auth via HttpOnly cookies** — tokens are never in JS (see below). `fetch` uses
  `credentials: 'include'`; the WS upgrade carries the cookie automatically.
- **Strict TypeScript** — no `any`, `@ts-ignore`, or `eslint-disable`; server events are a
  discriminated union driving a type-safe bus.
- **Query keys go through a single `QUERY_KEYS` factory** (`shared/constants`), so a query and
  its invalidation can never silently drift apart.
- **Message ordering is self-healing** — `messageCache.upsertMessage` keeps each page sorted
  by `created_at`, so out-of-order or late-confirmed realtime messages still render correctly.
- **Code-split by route** — pages are `React.lazy()`, so login/reset/add-instance don't pull
  in the heavy editor/messaging chunk.
- **Desktop** — the same SPA runs in an Electron shell (`electron/`) that adds native
  notifications, a dock badge, and `chatsystems://` deep links via a context-isolated preload.

## Authentication Model

Auth tokens are **never stored in JavaScript** (no localStorage, no Zustand). The backend sets them as `HttpOnly; SameSite=Lax` cookies on login/refresh, and the browser manages them automatically.

| Cookie | Path | Lifetime |
|--------|------|----------|
| `access_token` | `/` | `ACCESS_TOKEN_EXPIRY` (default 1h) |
| `refresh_token` | `/api/auth` | 7 days (rotated on each use) |

**Consequences for the frontend:**
- All `fetch` calls must use `credentials: 'include'`
- No token is passed to `WebSocketClient.connect()` — the browser sends the cookie on the WS upgrade handshake automatically
- On 401, `ApiClient` calls `POST /auth/refresh` (no body) to rotate tokens, then retries the original request
- The backend requires `CORS_ORIGINS` env var (comma-separated list of allowed frontend origins); wildcard `*` is not allowed when credentials are enabled

---

## Pages

### `WorkspacePage`
Main app container, kept thin: it calls `useWorkspaceController()` for all data and
handlers and renders the layout (sidebars, conversation, right panels). The logic lives
in the hook, not the view.

- **`features/workspace/hooks/useWorkspaceController`** — the "smart" hook: wires every
  query/store, the URL↔store sync effects, presence/notification init, WebSocket join +
  typing, and the send/upload/navigate handlers. Returns one object the view consumes.
- **`features/workspace/hooks/useRightPanel`** — open-panel state (members / settings /
  thread / search / pins / channel members / notifications), Esc-to-close, reset on
  conversation change.
- **`features/workspace/WorkspaceRightPanels`** — presentational; renders the active panel.

---

### `AddInstancePage`
Login page for connecting a new instance.

**Reads:** form input
**Writes:** `useAddInstance()` → POST `/auth/login` → saves instance (url + user, **no tokens**) to `useInstanceStore`

---

### `LoginPage` / `CompleteRegistrationPage` / `ResetPasswordPage`
Standard auth flows. POST to their respective `/auth/*` endpoints.

---

### `InstanceAdminPage`
Admin dashboard for the current instance (users, workspaces, stats).
**Reads:** GET `/admin/stats`, `/admin/users`, `/admin/workspaces`
**Writes:** suspend/activate users, hard-delete workspaces

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
Single message row with reactions, edit/delete, thread reply, and pin actions.

**Reads:** message, `useCurrentUser`, `useUserCache`, `useWorkspaceStore`
**API calls:**
- PATCH `/messages/{id}` — edit
- DELETE `/messages/{id}` — soft delete
- POST `/messages/{id}/reactions` — add reaction
- DELETE `/messages/{id}/reactions/{emoji}` — remove reaction
- POST `/messages/{id}/pin` — pin/unpin

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
Workspace members with roles and invite/remove actions.

**Reads:** `useWorkspaceMembers(workspaceId)`, current user role
**API calls:** POST/DELETE `/workspaces/{id}/members`, role update PATCH

---

### `SettingsPanel`
Workspace name, description, and deletion.

**API calls:** PATCH `/workspaces/{id}`, DELETE `/workspaces/{id}`

---

### `UserProfilePanel`
Edit display name, avatar, and password.

**API calls:** PATCH `/users/me`

---

### `TypingIndicator`
Shows "X is typing..." text. Listens for `typing.start` / `typing.stop` from the global event bus. No outbound data.

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
| `unreadChannels` | `Set<string>` | Channel IDs with unread messages |
| `mentionChannels` | `Set<string>` | Channel IDs with unread mentions |

Key methods:

| Method | What it does |
|--------|--------------|
| `selectWorkspace(ws)` | Sets current workspace + WS subscribe to workspace, sets active instance |
| `selectChannel(ch)` | Sets current channel + WS join channel |
| `setCurrentUserRole(role)` | Updates role in store |
| `markChannelRead(chId)` | Removes channel from unread/mention sets |

---

### `useInstanceStore`
Manages all connected instances and the active one. Persisted to localStorage (**no tokens in storage**).

| State | Description |
|-------|-------------|
| `instances` | Array of `{ url, user, wsUrl? }` — no `tokens` field |
| `activeInstanceUrl` | Currently active instance |

Key methods:

| Method | Description |
|--------|-------------|
| `addInstance(url, email, pass)` | POST `/auth/login`, saves `{ url, user }` to localStorage. Tokens are in HttpOnly cookies set by the server — never stored in JS. |
| `removeInstance(url)` | POST `/auth/logout`, disconnect WS, remove from store + localStorage |
| `setActiveInstance(url)` | Switch active instance |
| `restoreInstances()` | Re-hydrate `{ url, user }` from localStorage; call GET `/users/me` to validate session (401 → remove stale instance) |

---

### `usePresenceStore`
Maps `userId → 'online' \| 'away' \| 'offline'`. Updated by incoming WebSocket `presence.changed` / `presence.batch` events.

---

### `useUserCache`
In-memory map of `userId → { displayName, avatarUrl }`. Populated when workspace members load. Read by `MessageItem`, `ThreadPanel`, `ChannelSidebar`.

---

## React Query Hooks

All hooks accept an optional `instanceUrl` to target a specific connected instance.

### Auth
| Hook | Call | Description |
|------|------|-------------|
| `useCurrentUser()` | — | Returns user from `useInstanceStore` (no API call) |
| `useAddInstance()` | POST `/auth/login` | Login mutation; server sets `access_token` + `refresh_token` HttpOnly cookies; response body: `{ user, expires_in }` |
| `useCompleteRegistration()` | POST `/auth/complete-registration` | Same cookie-setting flow as login |
| `useLogout()` | POST `/auth/logout` | Server clears cookies; frontend clears store + query cache |
| `useRefreshSession()` | POST `/auth/refresh` | No request body; server reads `refresh_token` cookie, rotates it, sets new cookies; call on 401 before giving up |

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
| `useMessages(channelId)` | GET `/channels/{id}/messages?limit=50&cursor=...` | Infinite query, reversed pages |
| `useSendMessage(channelId, userId)` | POST `/channels/{id}/messages` | Optimistic mutation |
| `useEditMessage()` | PATCH `/messages/{id}` | Edit mutation |
| `useDeleteMessage()` | DELETE `/messages/{id}` | Soft-delete mutation |
| `useReactToMessage()` | POST `/messages/{id}/reactions` | Add reaction |
| `useRemoveReaction()` | DELETE `/messages/{id}/reactions/{emoji}` | Remove reaction |

### Threads
| Hook | Call | Description |
|------|------|-------------|
| `useThreadMessages(parentId)` | GET `/messages/{id}/thread` | Thread replies |
| `useSendThreadReply(parentId, chId)` | POST `/messages/{id}/thread` | Reply mutation |

### Channels
| Hook | Call | Description |
|------|------|-------------|
| `useChannel(channelId)` | GET `/channels/{id}` | Single channel |
| `useChannelMembers(channelId)` | GET `/channels/{id}/members` | Channel members |
| `useChannelPins(channelId)` | GET `/channels/{id}/pins` | Pinned messages |

### Notifications
| Hook | Call | Description |
|------|------|-------------|
| `useNotifications(wsId)` | GET `/workspaces/{id}/notifications?limit=50` | Notifications list |
| `useUnreadNotificationCount(wsId)` | GET `/workspaces/{id}/notifications/unread-count` | Unread count |
| `useMarkNotificationsRead(wsId)` | POST `/notifications/read` | Mark specific notifications read |
| `useMarkAllNotificationsRead(wsId)` | POST `/workspaces/{id}/notifications/read-all` | Mark all read |

---

## Infrastructure (`src/lib/`)

### `api.ts` — `ApiClient`
Thin fetch wrapper. All requests include `credentials: 'include'` so the browser sends the `access_token` HttpOnly cookie automatically. **No Authorization header is set or managed by the client.**
- Same-origin: routes through `/api` (Vite proxy)
- Cross-origin: routes to `{instanceUrl}/api`
- Methods: `get<T>`, `post<T>`, `patch<T>`, `delete<T>`
- On 401: automatically calls `POST /auth/refresh` once (token rotation via cookie), then retries; if refresh also fails, triggers logout
- Singleton `api` for default instance; `instanceManager.get(url).api` for others

### `ws.ts` — `WebSocketClient`
WebSocket wrapper with auto-reconnect (3s delay on disconnect).
- `connect()` — connects with no token argument; the browser sends the `access_token` cookie automatically on the WS upgrade handshake
- `send(event)` — sends JSON
- `subscribe(wsId)`, `joinChannel(chId)`, `leaveChannel(chId)` — convenience wrappers
- `on(type, handler)` — subscribe to event type (returns unsub fn)
- All received events are forwarded to `globalEventBus`
- Singleton `wsClient` for default; `instanceManager.get(url).ws` for others

### `instances.ts` — `InstanceManager`
Registry of `{ api: ApiClient, ws: WebSocketClient }` per instance URL.
- `get(url)` — get or create client pair
- `remove(url)` — disconnect WS + remove from registry
- Used by all hooks/stores that need instance-aware requests

### `globalEventBus.ts` — `GlobalEventBus`
Simple pub/sub bus. All WebSocket clients from all instances publish here.
- `on(type, handler)` — subscribe (returns unsub fn)
- `emit(event)` — broadcast

Event types: `message.new`, `message.updated`, `message.deleted`, `message.pinned`, `reaction.added`, `reaction.removed`, `notification`, `workspace.*`, `channel.*`, `member.*`, `presence.*`, `typing.*`

### `wsQuerySync.ts` — `useWebSocketQuerySync()`
Called once in `App.tsx`. Listens on `globalEventBus` and updates React Query cache.

| Event | Cache action |
|-------|-------------|
| `message.new` | Append to message list or confirm optimistic; increment `reply_count` on parent if thread reply |
| `message.updated` | Update content/updated_at in cached message |
| `message.deleted` | Soft-delete: set `deleted_at` on cached message |
| `message.pinned` | Invalidate pins query + update `is_pinned` on cached message |
| `reaction.added` / `reaction.removed` | Update reactions array on cached message |
| `channel.created` | Invalidate workspace channels list |
| `channel.updated` | Invalidate single channel query |
| `channel.member_added` / `channel.member_removed` | Invalidate channel members query |
| `member.added` / `member.removed` | Invalidate workspace members query |
| `workspace.created` / `workspace.deleted` / `workspace.restored` | Invalidate workspaces list |
| `workspace.updated` | Invalidate single workspace + workspaces list |

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
| Instance config (url + user, no tokens) | Zustand + localStorage | Login response `{ user }` |
| Auth tokens | HttpOnly cookies (server-managed) | Set by `/auth/login`, `/auth/refresh` |
| Current user | Zustand (from instance) | Login response |
| User display cache | Zustand | Populated from members list |
