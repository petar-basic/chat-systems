export const ROUTES = {
  addInstance: '/add-instance',
  completeRegistration: '/complete-registration',
  invite: '/invite/:token',
  forgotPassword: '/forgot-password',
  resetPassword: '/reset-password',
  app: '/app',
  admin: '/app/admin',

  workspace: (workspaceId: string) => `/app/${workspaceId}`,
  channel: (workspaceId: string, channelId: string) => `/app/${workspaceId}/${channelId}`,
  message: (workspaceId: string, channelId: string, messageId: string) =>
    `/app/${workspaceId}/${channelId}/${messageId}`,
  conversation: (workspaceId: string, conversationId: string) => `/app/${workspaceId}/c/${conversationId}`,
} as const;

export const ROUTE_PATTERNS = {
  workspaceOptional: '/app/:workspaceId?',
  channel: '/app/:workspaceId/:channelId',
  message: '/app/:workspaceId/:channelId/:messageId',
  conversation: '/app/:workspaceId/c/:conversationId',
} as const;
