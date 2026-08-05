import { useWorkspaceStore } from '@/stores/workspace';
import { useChannelMembers } from '@/hooks/queries/useChannels';
import { canModerateChannel, type ChannelRole } from '@/lib/channelPermissions';

export function useChannelModeration(channelId: string | null) {
  const workspaceRole = useWorkspaceStore((s) => s.currentUserRole);
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);
  const { data: members = [], isLoading } = useChannelMembers(channelId);

  const myRole = members.find((m) => m.user_id === currentUserId)?.role as ChannelRole | undefined;

  return {
    members,
    isLoading,
    currentUserId,
    workspaceRole,
    myRole,
    canModerate: canModerateChannel(workspaceRole, myRole),
  };
}
