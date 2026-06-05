export enum NotificationType {
  mention = 'mention',
  dm = 'dm',
  reply = 'reply',
  reaction = 'reaction',
  reminder = 'reminder',
  system = 'system',
}

export enum WorkspaceRole {
  owner = 'owner',
  admin = 'admin',
  channelAdmin = 'channel_admin',
  member = 'member',
  guest = 'guest',
}

export enum ChannelType {
  public = 'public',
  private = 'private',
}

export enum PresenceStatus {
  online = 'online',
  away = 'away',
  offline = 'offline',
}

export enum ConnectionStatus {
  connecting = 'connecting',
  connected = 'connected',
  disconnected = 'disconnected',
}

export enum MessageSource {
  channel = 'channel',
  dm = 'dm',
  thread = 'thread',
}

export enum ToastKind {
  success = 'success',
  error = 'error',
  info = 'info',
}
