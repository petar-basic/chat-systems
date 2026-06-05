export const ErrorLabels = {
  SendFailed: 'Message failed to send',
  EditFailed: "Couldn't save your edit",
  DeleteFailed: "Couldn't delete the message",
  ReactionFailed: "Couldn't update the reaction",
  PinFailed: "Couldn't update the pin",
  UploadFailed: 'File upload failed',
  RestoreFailed: "Couldn't restore the workspace",
  SessionExpired: 'Your session expired. Please sign in again.',
  NotFound: "We couldn't find that.",
} as const;

export const EmptyLabels = {
  NoMessages: 'No messages yet',
  NoMessagesHint: 'Be the first to say something!',
  NoNotifications: 'No notifications',
  NoNotificationsHint: "You're all caught up!",
  NoConversations: 'No conversations yet',
  NoChannels: 'No channels yet',
  NoChannelsHint: 'Create one to start the conversation.',
  DmBeginning: (name: string) => `This is the beginning of your conversation with ${name}.`,
} as const;

export const ConnectionLabels = {
  Connecting: 'Connecting…',
  Offline: "You're offline — reconnecting…",
  Reconnect: 'Reconnect',
} as const;

export const ActionLabels = {
  Retry: 'Retry',
  Cancel: 'Cancel',
  MarkAllRead: 'Mark all read',
  CreateChannel: 'Create Channel',
  LinkCopied: 'Link copied to clipboard',
} as const;

export const UNKNOWN_USER = 'Unknown user';
