# React Query Architecture

All query keys come from the single `QUERY_KEYS` factory in
`src/shared/constants/query-keys.ts` (the literals below mirror it).

## Query Keys Strategy

### 1. Workspaces
```typescript
['workspaces'] // List all workspaces
['workspaces', instanceUrls] // List query (keyed by joined instance URLs)
['workspaces', workspaceId] // Single workspace
['workspaces', workspaceId, 'members'] // Workspace members
['workspaces', workspaceId, 'channels'] // Workspace channels
['workspaces', 'deleted', instanceUrls] // Soft-deleted workspaces
```

**WS Events that invalidate:**
- `workspace.created` → invalidate `['workspaces']`
- `workspace.updated` → invalidate `['workspaces', workspaceId]` + `['workspaces']`
- `workspace.deleted` → invalidate `['workspaces']` + `['workspaces', 'deleted']` (+ navigate away unless workspace/instance admin)
- `workspace.restored` → invalidate `['workspaces']`
- `member.added` → invalidate `['workspaces', workspaceId, 'members']`
- `member.removed` → invalidate `['workspaces', workspaceId, 'members']`

### 2. Channels
```typescript
['channels', channelId] // Single channel
['channels', channelId, 'members'] // Channel members
['channels', channelId, 'pins'] // Pinned messages
['channels', 'unread', workspaceId] // Unread channel IDs
```

**WS Events that invalidate:**
- `channel.created` → invalidate `['workspaces', workspaceId, 'channels']`
- `channel.updated` → invalidate `['channels', channelId]`
- `channel.member_added` → invalidate `['channels', channelId, 'members']`
- `channel.member_removed` → invalidate `['channels', channelId, 'members']`

### 6. Notifications
```typescript
['notifications'] // Root key (invalidated on reconnect backfill)
['notifications', workspaceId] // Notifications list
['notifications', workspaceId, 'unread-count'] // Unread count
['notifications', 'dnd'] // Do-not-disturb window
```

**WS Events:** `notification` → handled by `NotificationStream` (not `wsQuerySync`): invalidates
`['notifications', workspaceId]` and `['notifications', workspaceId, 'unread-count']`, and marks
the channel unread/mention in `useWorkspaceStore`.

### 3. Messages
```typescript
['messages', channelId] // Channel messages (infinite query)
['threads', parentMessageId] // Thread replies
```

**WS Events that update:**
- `message.new` → **optimistic update** (confirm pending message or append from others; thread replies update `['threads', parentMessageId]` and increment `reply_count` on parent)
- `message.updated` → **optimistic update** (update content/updated_at in cache)
- `message.deleted` → **soft delete** (set `deleted_at` on cached message)
- `message.pinned` → invalidate `['channels', channelId, 'pins']` + update `is_pinned` in message cache

**Note:** Messages use **optimistic updates** instead of invalidation for instant UI

### 4. Reactions
```typescript
// Reactions are embedded in messages, no separate query needed
```

**WS Events that update:**
- `reaction.added` → **optimistic update** (add to message.reactions)
- `reaction.removed` → **optimistic update** (remove from message.reactions)

### 5. Presence
Presence is **not** in React Query — it lives entirely in the `usePresenceStore` Zustand store.

**WS Events that update the store:**
- `presence.changed` → direct Zustand update (with a 5s offline grace timer)
- `presence.batch` → bulk Zustand update

### 6. Search
```typescript
['search', query] // Search results
```
The `SearchPanel` component actually calls `api.get('/search?q=...&limit=20')` directly (debounced
local state), not a `useQuery` keyed by `['search', query]`; the key exists in `QUERY_KEYS` for
reuse.

**WS Events:** None (search is always fresh on query)

### 7. Direct Messages
```typescript
['dm'] // Root (invalidated on reconnect backfill)
['dm', 'conversations', workspaceId] // Conversation list
['dm', 'messages', workspaceId, partnerId] // Infinite DM thread
```

**WS Events that update:**
- `dm.new` → upsert into thread + conversation list (mark unread / notify if incoming)
- `dm.updated` / `dm.deleted` → patch / soft-delete
- `dm.reaction.added` / `dm.reaction.removed` → patch reactions

### 8. Huddles
```typescript
['huddles', 'active'] // Active huddles (root)
['huddles', 'active', workspaceId] // Active huddles for a workspace
```

**WS Events:** `huddle.started` / `huddle.ended` update the active-huddle map in `useWorkspaceStore`.

### 9. Auth
```typescript
['auth', 'currentUser'] // Reserved key; current user is read from useInstanceStore, not fetched
```

---

## Query Hooks Structure

All hooks (queries and mutations) live in `src/hooks/queries/` and use the TanStack Query v5
object API (`useQuery({ queryKey, queryFn, ... })`). Keys come from `QUERY_KEYS`.

```typescript
// src/hooks/queries/useAuth.ts
export const useCurrentUser = () => /* reads useInstanceStore, no fetch */
export const useAddInstance = () => useMutation(...)            // POST /auth/login (via store)
export const useCompleteRegistration = () => useMutation(...)   // POST /auth/complete-registration
export const useLogout = (instanceUrl?) => useMutation(...)     // POST /auth/logout

// src/hooks/queries/useWorkspaces.ts
export const useWorkspaces = () => useQuery({ queryKey: QUERY_KEYS.workspacesList(instanceUrls), ... })
export const useWorkspace = (id) => useQuery(...)
export const useWorkspaceMembers = (id, instanceUrl?) => useQuery(...)
export const useWorkspaceChannels = (id, instanceUrl?) => useQuery(...)
export const useDeletedWorkspaces = () => useQuery(...)
export const useRestoreWorkspace = () => useMutation(...)
export const useCreateWorkspace = () => useMutation(...)
export const useCreateChannel = () => useMutation(...)

// src/hooks/queries/useChannels.ts
export const useChannelMembers = (id) => useQuery(...)
export const useChannelPins = (id) => useQuery(...)
export const useUnreadChannelIds = (workspaceId, instanceUrl?) => useQuery(...)
export const useSetChannelMuted = (workspaceId, instanceUrl?) => useMutation(...)

// src/hooks/queries/useMessages.ts
export const useMessages = (channelId) => useInfiniteQuery(...)
export const useSendMessage = (channelId, userId) => useMutation(...)
export const useEditMessage = () => useMutation(...)
export const useDeleteMessage = () => useMutation(...)
export const useReactToMessage = () => useMutation(...)
export const useRemoveReaction = () => useMutation(...)
export const usePinMessage = () => useMutation(...)

// src/hooks/queries/useThreads.ts
export const useThreadMessages = (parentMessageId) => useQuery(...)
export const useSendThreadReply = (parentMessageId, channelId) => useMutation(...)

// src/hooks/queries/useDm.ts
export const useDmConversations = (workspaceId, instanceUrl?) => useQuery(...)
export const useDirectMessages = (workspaceId, partnerId, instanceUrl?) => useInfiniteQuery(...)
export const useSendDirectMessage = (...) => useMutation(...)
export const useEditDirectMessage / useDeleteDirectMessage = (...) => useMutation(...)
export const useReactToDm / useRemoveDmReaction = (...) => useMutation(...)
export const useMarkDmRead = (workspaceId, instanceUrl?) => useMutation(...)

// src/hooks/queries/useNotifications.ts
export const useNotifications = (workspaceId) => useQuery(...)
export const useUnreadNotificationCount = (workspaceId) => useQuery(...)
export const useWorkspaceUnreadCounts = (workspaces) => useQueries(...)
export const useMarkNotificationsRead = (workspaceId) => useMutation(...)
export const useMarkChannelNotificationsRead = (workspaceId) => useMutation(...)
export const useMarkAllNotificationsRead = (workspaceId) => useMutation(...)
export const useDndStatus / useSetDnd = () => useQuery / useMutation(...)

// src/hooks/queries/useHuddle.ts
export const useActiveHuddles = (workspaceId?, instanceUrl?) => useQuery(...)
```

---

## Mutations Structure

Mutations are co-located with their queries in `src/hooks/queries/`.

```typescript
// src/hooks/queries/useMessages.ts
export const useSendMessage = (channelId, userId) => useMutation({
  mutationFn: ({ content, id }) => api.post(`/channels/${channelId}/messages`, { content, id }),
  onMutate: async ({ content, id }) => {
    // Optimistic update: add pending message to cache immediately
    queryClient.setQueryData(['messages', channelId], (old) => ({
      ...old,
      pages: old.pages.map((page, i) =>
        i === old.pages.length - 1
          ? { ...page, data: [optimisticMessage, ...page.data] }
          : page
      ),
    }))
  },
  onError: (_err, { id }) => {
    // Mark the optimistic message as failed (kept in the list with a retry affordance)
    queryClient.setQueryData(['messages', channelId], (old) =>
      patchMessageById(old, id, (m) => ({ ...m, pending: false, failed: true })))
  },
  onSuccess: (_data, { id }) => {
    // Clear the failed flag; wsQuerySync flips `pending` off when the WS message.new arrives
    queryClient.setQueryData(['messages', channelId], (old) =>
      patchMessageById(old, id, (m) => ({ ...m, failed: false })))
  }
})

export const useEditMessage = () => useMutation({
  // onMutate: optimistic update (update content in cache immediately)
})

export const useDeleteMessage = () => useMutation({
  // onSuccess: soft delete (set deleted_at on cached message)
})

export const useReactToMessage = () => useMutation(...)
export const useRemoveReaction = () => useMutation(...)
```

---

## WebSocket Integration

`useWebSocketQuerySync()` is called once in `App.tsx`. It subscribes to `globalEventBus` (not `wsClient` directly — all WS clients from all instances forward events to the bus). The sketch below shows the channel-message reducer; the real `wsQuerySync.ts` also handles `dm.*` and `huddle.started`/`huddle.ended`. `presence.*`, `typing.indicator`, and `notification` are handled outside this file (`usePresenceStore`, `TypingIndicator`, `NotificationStream`).

```typescript
// src/lib/wsQuerySync.ts
import { useQueryClient } from '@tanstack/react-query'
import { globalEventBus } from './globalEventBus'

export const useWebSocketQuerySync = () => {
  const queryClient = useQueryClient()

  useEffect(() => {
    // Workspace events
    const unsubWorkspaceCreated = globalEventBus.on('workspace.created', () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces'] })
    })
    const unsubWorkspaceUpdated = globalEventBus.on('workspace.updated', (event) => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', event.workspace_id] })
      queryClient.invalidateQueries({ queryKey: ['workspaces'] })
    })
    const unsubWorkspaceDeleted = globalEventBus.on('workspace.deleted', (event) => {
      queryClient.invalidateQueries({ queryKey: ['workspaces'] })
      // Navigate away if currently viewing the deleted workspace
    })
    const unsubWorkspaceRestored = globalEventBus.on('workspace.restored', () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces'] })
    })

    // Member events
    const unsubMemberAdded = globalEventBus.on('member.added', (event) => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', event.workspace_id, 'members'] })
    })
    const unsubMemberRemoved = globalEventBus.on('member.removed', (event) => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', event.workspace_id, 'members'] })
    })

    // Channel events
    const unsubChannelCreated = globalEventBus.on('channel.created', (event) => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', event.workspace_id, 'channels'] })
    })
    const unsubChannelUpdated = globalEventBus.on('channel.updated', (event) => {
      queryClient.invalidateQueries({ queryKey: ['channels', event.channel_id] })
    })
    const unsubChannelMemberAdded = globalEventBus.on('channel.member_added', (event) => {
      queryClient.invalidateQueries({ queryKey: ['channels', event.channel_id, 'members'] })
    })
    const unsubChannelMemberRemoved = globalEventBus.on('channel.member_removed', (event) => {
      queryClient.invalidateQueries({ queryKey: ['channels', event.channel_id, 'members'] })
    })

    // Message events — optimistic cache updates (no invalidation)
    const unsubMessageNew = globalEventBus.on('message.new', (event) => {
      const message = event.message
      if (!message?.channel_id) return

      // Thread reply: update thread cache + increment reply_count on parent
      if (message.thread_parent_id) {
        queryClient.setQueryData(['threads', message.thread_parent_id], (old = []) => {
          if (old.some(m => m.id === message.id)) return old
          return [...old, message]
        })
        // Increment reply_count on parent message in channel cache
        queryClient.setQueryData(['messages', message.channel_id], (old) => { /* ... */ })
        return
      }

      queryClient.setQueryData(['messages', message.channel_id], (old) => {
        if (!old) return old
        const exists = old.pages.some(page => page.data.some(m => m.id === message.id))
        if (exists) {
          // Confirm optimistic message (set pending: false)
          return { ...old, pages: old.pages.map(page => ({
            ...page,
            data: page.data.map(m => m.id === message.id ? { ...message, pending: false } : m)
          }))}
        }
        // Append message from another user (prepend to last page — newest first)
        return { ...old, pages: old.pages.map((page, i) =>
          i === old.pages.length - 1 ? { ...page, data: [message, ...page.data] } : page
        )}
      })
    })

    const unsubMessageUpdated = globalEventBus.on('message.updated', (event) => {
      const message = event.message
      queryClient.setQueryData(['messages', message.channel_id], (old) => {
        if (!old) return old
        return { ...old, pages: old.pages.map(page => ({
          ...page,
          data: page.data.map(m => m.id === message.id
            ? { ...m, content: message.content, updated_at: message.updated_at }
            : m)
        }))}
      })
    })

    const unsubMessageDeleted = globalEventBus.on('message.deleted', (event) => {
      // Soft delete — set deleted_at (do NOT remove from array)
      queryClient.setQueryData(['messages', event.channel_id], (old) => {
        if (!old) return old
        return { ...old, pages: old.pages.map(page => ({
          ...page,
          data: page.data.map(m => m.id === event.message_id
            ? { ...m, deleted_at: new Date().toISOString() }
            : m)
        }))}
      })
    })

    // Reaction events
    const unsubReactionAdded = globalEventBus.on('reaction.added', (event) => {
      const channelId = event.reaction?.channel_id
      if (!channelId) return
      queryClient.setQueryData(['messages', channelId], (old) => {
        if (!old) return old
        return { ...old, pages: old.pages.map(page => ({
          ...page,
          data: page.data.map(m => m.id === event.message_id
            ? { ...m, reactions: [...(m.reactions || []), event.reaction] }
            : m)
        }))}
      })
    })

    const unsubReactionRemoved = globalEventBus.on('reaction.removed', (event) => {
      queryClient.setQueryData(['messages', event.channel_id], (old) => {
        if (!old) return old
        return { ...old, pages: old.pages.map(page => ({
          ...page,
          data: page.data.map(m => m.id === event.message_id
            ? { ...m, reactions: (m.reactions || []).filter(r =>
                !(r.user_id === event.user_id && r.emoji === event.emoji)) }
            : m)
        }))}
      })
    })

    // Pin events
    const unsubMessagePinned = globalEventBus.on('message.pinned', (event) => {
      queryClient.invalidateQueries({ queryKey: ['channels', event.channel_id, 'pins'] })
      queryClient.setQueryData(['messages', event.channel_id], (old) => {
        if (!old) return old
        return { ...old, pages: old.pages.map(page => ({
          ...page,
          data: page.data.map(m => m.id === event.message_id
            ? { ...m, is_pinned: event.pinned }
            : m)
        }))}
      })
    })

    return () => { /* unsubscribe all */ }
  }, [queryClient])
}
```

---

## Benefits

1. **No more manual state management** — React Query handles caching
2. **Automatic deduplication** — Multiple components can use same query
3. **Background refetching** — Data stays fresh
4. **Optimistic updates** — Instant UI for messages/reactions
5. **WS-driven invalidation** — Real-time sync without polling
6. **Infinite scroll** — Built-in with `useInfiniteQuery`
7. **Loading/error states** — Automatic from React Query

---

## Implementation Status

Migration to React Query is **complete**. The architecture described in this document is the current production state:

- `QueryClientProvider` is configured in `App.tsx` with `ReactQueryDevtools`
- All query and mutation hooks are in `src/hooks/queries/`
- `useWebSocketQuerySync()` is called once in `App.tsx`
- Zustand stores hold UI/session state: `useWorkspaceStore`, `usePresenceStore`, `useUserCache`, plus `drafts`, `notificationPrefs`, `wsStatus`, and `huddle`. `useInstanceStore` is the exception — it owns the connected-instance list and the logged-in user (and, cross-origin, the persisted tokens)
- React Query owns server data (workspaces, channels, messages, threads, DMs, notifications, active huddles)
