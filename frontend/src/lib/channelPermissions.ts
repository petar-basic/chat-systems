import type { ChannelSettings } from '@/stores/workspace';
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

/**
 * Mirrors `ChannelAccess::can_post` on the server, and deliberately delegates to
 * `canModerateChannel` rather than restating who a moderator is — one copy of
 * that rule on this side, not two.
 *
 * The server is still the boundary. This exists so the composer does not offer
 * what will be refused, which reads as the product being broken rather than as
 * the channel being locked.
 */
export function canPostInChannel(
  settings: ChannelSettings | null | undefined,
  workspaceRole: WorkspaceRole | null,
  channelRole: ChannelRole | undefined,
): boolean {
  if (settings?.post_policy !== 'moderators') return true;
  return canModerateChannel(workspaceRole, channelRole);
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
