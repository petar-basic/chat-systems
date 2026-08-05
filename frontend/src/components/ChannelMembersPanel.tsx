import { useState } from 'react';
import { X, UserPlus, UserMinus, Hash, ShieldCheck, Shield } from 'lucide-react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';
import { instanceManager } from '../lib/instances';
import { useUserCache } from '../stores/users';
import { useWorkspaceStore } from '../stores/workspace';
import { useUpdateChannelMemberRole } from '../hooks/queries/useChannels';
import { useChannelModeration } from '@/features/channel/hooks/useChannelModeration';
import { canAddChannelMembers, type ChannelRole } from '@/lib/channelPermissions';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { QUERY_KEYS } from '@/shared/constants';

interface ChannelMember {
  id: string;
  channel_id: string;
  user_id: string;
  role: string;
  joined_at: string;
}

interface Props {
  channelId: string;
  channelName: string;
  channelType: string;
  onClose: () => void;
}

function MemberRow({
  member,
  isSelf,
  canModerate,
  busy,
  onSetRole,
  onRemove,
}: {
  member: ChannelMember;
  isSelf: boolean;
  canModerate: boolean;
  busy: boolean;
  onSetRole: (userId: string, role: ChannelRole) => void;
  onRemove: (userId: string) => void;
}) {
  const { getUser } = useUserCache();
  const info = getUser(member.user_id);
  const displayName = info?.display_name || member.user_id.slice(0, 8);
  const isAdmin = member.role === 'admin';

  return (
    <div
      className="flex items-center gap-3 px-4 py-2 hover:bg-slate-700/30"
      data-qa="channel-member-row"
      data-user-id={member.user_id}
    >
      <Avatar userId={member.user_id} name={displayName} avatarUrl={info?.avatar_url} />
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium truncate">
          {displayName}
          {isSelf && <span className="text-slate-400 font-normal"> (you)</span>}
        </div>
        <div className="text-xs text-slate-400 flex items-center gap-1">
          {isAdmin && <ShieldCheck className="w-3 h-3 text-blue-400" />}
          <span data-qa="channel-member-role">{isAdmin ? 'Channel admin' : 'Member'}</span>
        </div>
      </div>
      {canModerate && (
        <button
          onClick={() => onSetRole(member.user_id, isAdmin ? 'member' : 'admin')}
          disabled={busy}
          aria-label={
            isAdmin ? `Remove channel admin from ${displayName}` : `Make ${displayName} a channel admin`
          }
          title={isAdmin ? 'Remove channel admin' : 'Make channel admin'}
          data-qa={isAdmin ? 'channel-member-demote' : 'channel-member-promote'}
          className={`p-1 rounded transition cursor-pointer disabled:opacity-50 ${
            isAdmin ? 'text-blue-400 hover:text-slate-300' : 'text-slate-400 hover:text-blue-400'
          }`}
        >
          <Shield className="w-4 h-4" />
        </button>
      )}
      {(canModerate || isSelf) && (
        <button
          onClick={() => onRemove(member.user_id)}
          disabled={busy}
          aria-label={isSelf ? 'Leave channel' : `Remove ${displayName} from channel`}
          title={isSelf ? 'Leave channel' : 'Remove from channel'}
          data-qa="channel-member-remove"
          className="text-slate-400 hover:text-red-400 transition cursor-pointer p-1 disabled:opacity-50"
        >
          <UserMinus className="w-4 h-4" />
        </button>
      )}
    </div>
  );
}

function AddableUserRow({ userId, onAdd }: { userId: string; onAdd: (id: string) => void }) {
  const { getUser } = useUserCache();
  const info = getUser(userId);
  const displayName = info?.display_name || userId.slice(0, 8);

  return (
    <button
      onClick={() => onAdd(userId)}
      data-qa="channel-member-add"
      data-user-id={userId}
      className="w-full flex items-center gap-3 px-4 py-2 hover:bg-slate-700/30 cursor-pointer text-left"
    >
      <Avatar userId={userId} name={displayName} avatarUrl={info?.avatar_url} size="sm" />
      <span className="text-sm truncate">{displayName}</span>
      <UserPlus className="w-3.5 h-3.5 text-slate-400 ml-auto shrink-0" />
    </button>
  );
}

export default function ChannelMembersPanel({ channelId, channelName, channelType, onClose }: Props) {
  const { users } = useUserCache();
  const {
    members,
    isLoading: loading,
    canModerate,
    myRole,
    workspaceRole,
    currentUserId,
  } = useChannelModeration(channelId);
  const [showAddUser, setShowAddUser] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);
  const apiClient = currentWorkspace?.instanceUrl
    ? instanceManager.get(currentWorkspace.instanceUrl).api
    : api;

  const canAdd = canAddChannelMembers(workspaceRole, myRole, channelType);

  const failWith = (fallback: string) => (err: unknown) =>
    setError((err as { message?: string })?.message || fallback);

  const addMemberMutation = useMutation({
    mutationFn: async (userId: string) => {
      return apiClient.post<ChannelMember>(`/channels/${channelId}/members`, { user_id: userId });
    },
    onSuccess: () => {
      setError(null);
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelMembers(channelId) });
    },
    onError: failWith('Failed to add member'),
  });

  const removeMemberMutation = useMutation({
    mutationFn: async (userId: string) => {
      return apiClient.delete(`/channels/${channelId}/members/${userId}`);
    },
    onSuccess: () => {
      setError(null);
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelMembers(channelId) });
    },
    onError: failWith('Failed to remove member'),
  });

  const roleMutation = useUpdateChannelMemberRole(channelId);

  const channelMemberIds = new Set(members.map((m) => m.user_id));
  const nonMembers = Array.from(users.keys()).filter((id) => !channelMemberIds.has(id));
  const busy = addMemberMutation.isPending || removeMemberMutation.isPending || roleMutation.isPending;

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-slate-800 border-l border-slate-700/50 flex flex-col"
      data-qa="channel-members-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700/50 shrink-0">
        <div className="flex items-center gap-2">
          <Hash className="w-4 h-4 text-slate-400" />
          <span className="font-semibold truncate">{channelName} Members</span>
        </div>
        <button onClick={onClose} className="text-slate-400 hover:text-white transition cursor-pointer">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : (
          <>
            <div className="px-4 py-2 flex items-center justify-between">
              <span className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
                Members ({members.length})
              </span>
              {canAdd && (
                <button
                  onClick={() => setShowAddUser(!showAddUser)}
                  className="text-slate-400 hover:text-white transition cursor-pointer"
                  aria-label="Add member"
                  data-qa="channel-members-add-toggle"
                  title="Add member"
                >
                  <UserPlus className="w-4 h-4" />
                </button>
              )}
            </div>
            {error && <div className="px-4 pb-1 text-xs text-red-400">{error}</div>}
            {members.map((m) => (
              <MemberRow
                key={m.user_id}
                member={m}
                isSelf={m.user_id === currentUserId}
                canModerate={canModerate}
                busy={busy}
                onSetRole={(userId, role) => roleMutation.mutate({ userId, role })}
                onRemove={(id) => removeMemberMutation.mutate(id)}
              />
            ))}
          </>
        )}
      </div>

      {showAddUser && canAdd && nonMembers.length > 0 && (
        <div className="border-t border-slate-700/50 max-h-48 overflow-y-auto">
          <div className="px-4 py-2 text-xs font-semibold text-slate-400 uppercase tracking-wider">
            Add to channel
          </div>
          {nonMembers.map((userId) => (
            <AddableUserRow key={userId} userId={userId} onAdd={(id) => addMemberMutation.mutate(id)} />
          ))}
        </div>
      )}
    </div>
  );
}
