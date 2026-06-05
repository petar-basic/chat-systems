# React Query Architecture

## Query Keys Strategy

### 1. Workspaces
```typescript
['workspaces'] // List all workspaces
['workspaces', workspaceId] // Single workspace
['workspaces', workspaceId, 'members'] // Workspace members
['workspaces', workspaceId, 'channels'] // Workspace channels
```

**WS Events that invalidate:**
- `workspace.created` → invalidate `['workspaces']`
- `workspace.updated` → invalidate `['workspaces', workspaceId]` + `['workspaces']`
- `workspace.deleted` → invalidate `['workspaces']` (+ navigate away if current workspace)
- `workspace.restored` → invalidate `['workspaces']`
- `member.added` → invalidate `['workspaces', workspaceId, 'members']`
- `member.removed` → invalidate `['workspaces', workspaceId, 'members']`
- `member.role_changed` → invalidate `['workspaces', workspaceId, 'members']`

### 2. Channels
```typescript
['channels', channelId] // Single channel
['channels', channelId, 'members'] // Channel members
['channels', channelId, 'pins'] // Pinned messages
```

**WS Events that invalidate:**
- `channel.created` → invalidate `['workspaces', workspaceId, 'channels']`
- `channel.updated` → invalidate `['channels', channelId]`
- `channel.deleted` → invalidate `['workspaces', workspaceId, 'channels']`
- `channel.member_added` → invalidate `['channels', channelId, 'members']`
- `channel.member_removed` → invalidate `['channels', channelId, 'members']`

### 6. Notifications
```typescript
['notifications', workspaceId] // Notifications list
['notifications', workspaceId, 'unread-count'] // Unread count
```

**WS Events:** None (notifications are fetched on demand or polled)

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
```typescript
['presence', workspaceId] // Workspace presence
```

**WS Events that update:**
- `presence.changed` → **direct state update** (too frequent for query invalidation)

### 6. Search
```typescript
['search', query, filters] // Search results
```

**WS Events:** None (search is always fresh on query)

---

## Query Hooks Structure

All hooks (queries and mutations) live in `src/hooks/queries/`.

```typescript
// src/hooks/queries/useWorkspaces.ts
export const useWorkspaces = () => useQuery(['workspaces', instanceUrls], ...)
export const useWorkspace = (id) => useQuery(['workspaces', id], ...)
export const useWorkspaceMembers = (id, instanceUrl?) => useQuery(['workspaces', id, 'members'], ...)
export const useWorkspaceChannels = (id, instanceUrl?) => useQuery(['workspaces', id, 'channels'], ...)
export const useDeletedWorkspaces = () => useQuery(['workspaces', 'deleted', instanceUrls], ...)
export const useRestoreWorkspace = () => useMutation(...)
export const useCreateWorkspace = () => useMutation(...)
export const useCreateChannel = () => useMutation(...)

// src/hooks/queries/useChannels.ts
export const useChannel = (id) => useQuery(['channels', id], ...)
export const useChannelMembers = (id) => useQuery(['channels', id, 'members'], ...)
export const useChannelPins = (id) => useQuery(['channels', id, 'pins'], ...)

// src/hooks/queries/useMessages.ts
export const useMessages = (channelId) => useInfiniteQuery(['messages', channelId], ...)
export const useSendMessage = (channelId, userId) => useMutation(...)
export const useEditMessage = () => useMutation(...)
export const useDeleteMessage = () => useMutation(...)
export const useReactToMessage = () => useMutation(...)
export const useRemoveReaction = () => useMutation(...)

// src/hooks/queries/useThreads.ts
export const useThreadMessages = (parentMessageId) => useQuery(['threads', parentMessageId], ...)
export const useSendThreadReply = (parentMessageId, channelId) => useMutation(...)

// src/hooks/queries/useNotifications.ts
export const useNotifications = (workspaceId) => useQuery(['notifications', workspaceId], ...)
export const useUnreadNotificationCount = (workspaceId) => useQuery(['notifications', workspaceId, 'unread-count'], ...)
export const useMarkNotificationsRead = (workspaceId) => useMutation(...)
export const useMarkAllNotificationsRead = (workspaceId) => useMutation(...)
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
    // Rollback: remove the optimistic message
    queryClient.setQueryData(['messages', channelId], (old) => ({
      ...old,
      pages: old.pages.map((page) => ({
        ...page,
        data: page.data.filter((m) => m.id !== id),
      })),
    }))
  }
  // No onSuccess needed — wsQuerySync confirms the message when WS event arrives
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

`useWebSocketQuerySync()` is called once in `App.tsx`. It subscribes to `globalEventBus` (not `wsClient` directly — all WS clients from all instances forward events to the bus).

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
- Zustand stores (`useWorkspaceStore`, `useInstanceStore`, `usePresenceStore`, `useUserCache`) hold **UI-only state** — no server data
- React Query owns all server data (workspaces, channels, messages, threads, notifications)
