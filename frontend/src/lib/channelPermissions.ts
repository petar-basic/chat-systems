import type { ChannelSettings } from '@/stores/workspace';
import type { WorkspaceRole } from '@/stores/workspace';
import { allows, ruleOf, type WhoMay } from './workspaceRules';

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

export type ChannelKind = 'public' | 'private';

export function canBroadcastInChannel(
  settings: ChannelSettings | null | undefined,
  rule: WhoMay,
  workspaceRole: WorkspaceRole | null,
  channelRole: ChannelRole | undefined,
): boolean {
  if (settings?.allow_broadcast_mentions === false) return false;
  if (rule === 'members') return true;
  return workspaceRole === 'owner' || workspaceRole === 'admin' || channelRole === 'admin';
}

export function creatableChannelKinds(
  workspaceRole: WorkspaceRole | null,
  workspaceSettings: unknown,
): ChannelKind[] {
  return (['public', 'private'] as const).filter((kind) =>
    allows(ruleOf(workspaceSettings, `${kind}_channel_creators` as const), workspaceRole),
  );
}
