export const QUERY_KEYS = {
  currentUser: () => ['auth', 'currentUser'] as const,

  workspaces: () => ['workspaces'] as const,
  workspacesList: (instanceUrls: string) => ['workspaces', instanceUrls] as const,
  workspace: (id: string) => ['workspaces', id] as const,
  workspaceMembers: (id: string) => ['workspaces', id, 'members'] as const,
  workspaceChannels: (id: string) => ['workspaces', id, 'channels'] as const,
  deletedWorkspaces: () => ['workspaces', 'deleted'] as const,
  deletedWorkspacesList: (instanceUrls: string) => ['workspaces', 'deleted', instanceUrls] as const,

  channels: () => ['channels'] as const,
  channel: (id: string) => ['channels', id] as const,
  channelMembers: (id: string) => ['channels', id, 'members'] as const,
  channelPins: (id: string) => ['channels', id, 'pins'] as const,
  channelsUnread: (workspaceId: string) => ['channels', 'unread', workspaceId] as const,
  channelsBrowse: (workspaceId: string) => ['channels', 'browse', workspaceId] as const,

  messagesAll: () => ['messages'] as const,
  messages: (channelId: string) => ['messages', channelId] as const,
  thread: (parentId: string) => ['threads', parentId] as const,

  notificationsAll: () => ['notifications'] as const,
  notifications: (workspaceId: string) => ['notifications', workspaceId] as const,
  notificationUnreadCount: (workspaceId: string) => ['notifications', workspaceId, 'unread-count'] as const,
  notificationDnd: () => ['notifications', 'dnd'] as const,

  conversationsAll: () => ['conversations'] as const,
  conversations: (workspaceId: string) => ['conversations', workspaceId] as const,
  conversationMessages: (conversationId: string) => ['conversations', 'messages', conversationId] as const,

  scheduledMessages: (workspaceId: string) => ['scheduled-messages', workspaceId] as const,

  search: (query: string) => ['search', query] as const,

  hooks: (workspaceId: string) => ['hooks', workspaceId] as const,
  hookedChannels: (workspaceId: string) => ['hooks', workspaceId, 'channels'] as const,

  auditLog: (workspaceId: string) => ['audit-log', workspaceId] as const,

  editHistory: (messageId: string) => ['edit-history', messageId] as const,

  huddlesActive: () => ['huddles', 'active'] as const,
  workspaceActiveHuddles: (workspaceId: string) => ['huddles', 'active', workspaceId] as const,
} as const;
