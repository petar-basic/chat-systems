export const QUERY_KEYS = {
  currentUser: () => ['auth', 'currentUser'] as const,
  instanceInfo: (instanceUrl: string) => ['instance', 'info', instanceUrl] as const,
  myStatus: (instanceUrl: string) => ['auth', 'status', instanceUrl] as const,

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
  channelFiles: (id: string, kind: string) => ['channels', id, 'files', kind] as const,
  channelFileSearch: (id: string, name: string) => ['channels', id, 'files', 'search', name] as const,
  channelsUnread: (workspaceId: string) => ['channels', 'unread', workspaceId] as const,
  channelsBrowse: (workspaceId: string) => ['channels', 'browse', workspaceId] as const,

  messagesAll: () => ['messages'] as const,
  messages: (channelId: string) => ['messages', channelId] as const,
  thread: (parentId: string) => ['threads', parentId] as const,
  followedThreads: (workspaceId: string) => ['followed-threads', workspaceId] as const,
  unreadThreads: (workspaceId: string) => ['followed-threads', workspaceId, 'unread'] as const,
  threadState: (rootId: string) => ['thread-state', rootId] as const,

  notificationsAll: () => ['notifications'] as const,
  notifications: (workspaceId: string) => ['notifications', workspaceId] as const,
  notificationUnreadCount: (workspaceId: string) => ['notifications', workspaceId, 'unread-count'] as const,
  notificationDnd: () => ['notifications', 'dnd'] as const,
  notificationSchedule: () => ['notifications', 'schedule'] as const,
  notifyDefault: (workspaceId: string) => ['notify-default', workspaceId] as const,
  notificationKeywords: (workspaceId: string) => ['notification-keywords', workspaceId] as const,

  conversationsAll: () => ['conversations'] as const,
  conversations: (workspaceId: string) => ['conversations', workspaceId] as const,
  unreads: (workspaceId: string) => ['unreads', workspaceId] as const,
  sidebar: (workspaceId: string) => ['sidebar', workspaceId] as const,
  apps: (workspaceId: string) => ['apps', workspaceId] as const,

  scheduledMessages: (workspaceId: string) => ['scheduled-messages', workspaceId] as const,

  slackImports: (workspaceId: string) => ['slack-imports', workspaceId] as const,
  later: (workspaceId: string, status?: string) =>
    (status ? ['later', workspaceId, status] : ['later', workspaceId]) as readonly string[],
  channelBookmarks: (channelId: string) => ['channels', channelId, 'bookmarks'] as const,

  search: (query: string) => ['search', query] as const,

  hooks: (workspaceId: string) => ['hooks', workspaceId] as const,
  hookedChannels: (workspaceId: string) => ['hooks', workspaceId, 'channels'] as const,

  auditLog: (workspaceId: string) => ['audit-log', workspaceId] as const,
  retention: (workspaceId: string) => ['retention', workspaceId] as const,
  retentionPreview: (
    workspaceId: string,
    draft: { messageDays: number | null; fileDays: number | null; channelOverrides: boolean },
  ) => ['retention', workspaceId, 'preview', draft] as const,
  channelRetention: (channelId: string) => ['channels', channelId, 'retention'] as const,
  exports: (workspaceId: string) => ['exports', workspaceId] as const,
  scimTokens: (instanceUrl: string) => ['admin', 'scim-tokens', instanceUrl] as const,
  adminUsers: (instanceUrl: string) => ['admin', 'users', instanceUrl] as const,
  adminAuditLog: (instanceUrl: string) => ['admin', 'audit-log', instanceUrl] as const,

  editHistory: (messageId: string) => ['edit-history', messageId] as const,

  pollVote: (pollId: string) => ['polls', pollId, 'vote'] as const,
  pollVoters: (pollId: string, tally: string) => ['polls', pollId, 'voters', tally] as const,

  channelLists: (workspaceId: string, channelId: string) => ['lists', 'of', workspaceId, channelId] as const,
  listsOf: (workspaceId: string) => ['lists', 'of', workspaceId] as const,
  list: (listId: string) => ['lists', listId, 'detail'] as const,
  listItems: (listId: string) => ['lists', listId, 'items'] as const,
  listComments: (itemId: string) => ['lists', 'comments', itemId] as const,

  workflows: (workspaceId: string) => ['workflows', workspaceId] as const,
  workflowRuns: (workflowId: string) => ['workflows', 'runs', workflowId] as const,
  workflowShortcuts: (channelId: string) => ['workflows', 'shortcuts', channelId] as const,

  huddlesActive: () => ['huddles', 'active'] as const,
  workspaceActiveHuddles: (workspaceId: string) => ['huddles', 'active', workspaceId] as const,
} as const;
