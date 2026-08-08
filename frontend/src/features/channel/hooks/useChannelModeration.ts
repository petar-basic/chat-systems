import { useCurrentUser } from '@/hooks/queries/useAuth';
import { useChannelMembers } from '@/hooks/queries/useChannels';
import { useCurrentWorkspaceRole } from '@/features/workspace/hooks/useCurrentWorkspaceRole';
import { canModerateChannel, type ChannelRole } from '@/lib/channelPermissions';

export function useChannelModeration(channelId: string | null) {
  const { data: user } = useCurrentUser();
  const { role: workspaceRole, isResolved: roleResolved } = useCurrentWorkspaceRole();
  const { data: members = [], isLoading } = useChannelMembers(channelId);

  const currentUserId = user?.id ?? null;
  const myRole = members.find((m) => m.user_id === currentUserId)?.role as ChannelRole | undefined;
  const resolved = roleResolved && !!currentUserId && !isLoading;

  return {
    members,
    isLoading,
    currentUserId,
    workspaceRole,
    myRole,
    resolved,
    canModerate: resolved && canModerateChannel(workspaceRole, myRole),
  };
}
