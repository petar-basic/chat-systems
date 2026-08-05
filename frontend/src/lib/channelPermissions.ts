import type { WorkspaceRole } from '@/stores/workspace';

export type ChannelRole = 'member' | 'admin';

export function canModerateChannel(
  workspaceRole: WorkspaceRole | null,
  channelRole: ChannelRole | undefined,
): boolean {
  if (workspaceRole === 'owner' || workspaceRole === 'admin') return true;
  if (!workspaceRole || workspaceRole === 'guest') return false;
  return channelRole === 'admin';
}

export function canAddChannelMembers(
  workspaceRole: WorkspaceRole | null,
  channelRole: ChannelRole | undefined,
  channelType: string,
): boolean {
  if (!workspaceRole || workspaceRole === 'guest') return false;
  if (canModerateChannel(workspaceRole, channelRole)) return true;
  return channelType === 'public' || channelRole !== undefined;
}
