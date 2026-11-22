export const ErrorLabels = {
  MarkReadFailed: "Couldn't mark the channel read",
  MarkAllReadFailed: "Couldn't mark everything read",
  SidebarSaveFailed: "Couldn't save your sidebar",
  AppSaveFailed: "Couldn't save the app",
  CalendarSaveFailed: "Couldn't save the calendar settings",
  LaterFailed: "Couldn't update Later",
  RecentSearchesFailed: "Couldn't change your recent searches",
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
  DmBeginning: (name: string) => `This is the beginning of your conversation with ${name}.`,
} as const;

export const ActionLabels = {
  Cancel: 'Cancel',
  MarkedAllRead: 'Everything is marked read',
  CreateChannel: 'Create channel',
} as const;
