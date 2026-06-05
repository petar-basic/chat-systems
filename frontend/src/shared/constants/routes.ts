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
  dm: (workspaceId: string, partnerId: string) => `/app/${workspaceId}/dm/${partnerId}`,
} as const;

export const ROUTE_PATTERNS = {
  workspaceOptional: '/app/:workspaceId?',
  channel: '/app/:workspaceId/:channelId',
  message: '/app/:workspaceId/:channelId/:messageId',
  dm: '/app/:workspaceId/dm/:dmUserId',
} as const;
