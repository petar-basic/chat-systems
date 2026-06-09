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

  messagesAll: () => ['messages'] as const,
  messages: (channelId: string) => ['messages', channelId] as const,
  thread: (parentId: string) => ['threads', parentId] as const,

  dm: () => ['dm'] as const,
  dmConversations: (workspaceId: string) => ['dm', 'conversations', workspaceId] as const,
  dmMessages: (workspaceId: string, partnerId: string) => ['dm', 'messages', workspaceId, partnerId] as const,

  notificationsAll: () => ['notifications'] as const,
  notifications: (workspaceId: string) => ['notifications', workspaceId] as const,
  notificationUnreadCount: (workspaceId: string) => ['notifications', workspaceId, 'unread-count'] as const,
  notificationDnd: () => ['notifications', 'dnd'] as const,

  search: (query: string) => ['search', query] as const,

  huddlesActive: () => ['huddles', 'active'] as const,
  workspaceActiveHuddles: (workspaceId: string) => ['huddles', 'active', workspaceId] as const,
} as const;
