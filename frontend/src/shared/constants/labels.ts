export const ErrorLabels = {
  MarkReadFailed: "Couldn't mark the channel read",
  MarkAllReadFailed: "Couldn't mark everything read",
  SidebarSaveFailed: "Couldn't save your sidebar",
  AppSaveFailed: "Couldn't save the app",
  LaterFailed: "Couldn't update Later",
  ListCellFailed: "Couldn't save that cell",
  ListItemFailed: "Couldn't change that item",
  ListColumnFailed: "Couldn't change that column",
  ListFailed: "Couldn't change the list",
  ListViewFailed: "Couldn't change that view",
  ListCommentFailed: "Couldn't save the comment",
  AppConnectFailed: "Couldn't start connecting the app",
  UploadFailed: 'File upload failed',
  RestoreFailed: "Couldn't restore the workspace",
  SessionExpired: 'Your session expired. Please sign in again.',
  SessionRevoked: 'This session was ended. Please sign in again.',
  NotFound: "We couldn't find that.",
} as const;

export const EmptyLabels = {
  NoNotifications: 'No notifications',
  NoNotificationsHint: "You're all caught up!",
  NoChannels: 'No channels yet',
  NoChannelsHint: 'Create one to start the conversation.',
  NoWorkspaces: "You're not in a workspace yet",
  NoWorkspacesHint: 'Create one with + in the sidebar, or ask someone to invite you.',
  NoWorkspacesInviteHint: 'Once someone invites you to one, it shows up here.',
  DmBeginning: (name: string) => `This is the beginning of your conversation with ${name}.`,
} as const;

export const ActionLabels = {
  Cancel: 'Cancel',
  MarkAllRead: 'Mark all read',
  MarkedAllRead: 'Everything is marked read',
  CreateChannel: 'Create channel',
} as const;
