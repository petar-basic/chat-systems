import { useState } from 'react';
import { X, UserPlus, UserMinus, Hash } from 'lucide-react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';
import { instanceManager } from '../lib/instances';
import { useUserCache } from '../stores/users';
import { useWorkspaceStore } from '../stores/workspace';
import { useChannelMembers } from '../hooks/queries/useChannels';
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
  onClose: () => void;
}

const AVATAR_COLORS = [
  'bg-purple-600',
  'bg-blue-600',
  'bg-green-600',
  'bg-amber-600',
  'bg-pink-600',
  'bg-teal-600',
  'bg-indigo-600',
  'bg-rose-600',
];

function getColorIndex(userId: string) {
  return userId.split('').reduce((acc, ch) => acc + ch.charCodeAt(0), 0) % AVATAR_COLORS.length;
}

function MemberRow({ member, onRemove }: { member: ChannelMember; onRemove: (userId: string) => void }) {
  const { getUser } = useUserCache();
  const info = getUser(member.user_id);
  const displayName = info?.display_name || member.user_id.slice(0, 8);
  const initials = displayName.charAt(0).toUpperCase();
  const avatarColor = AVATAR_COLORS[getColorIndex(member.user_id)];

  return (
    <div className="flex items-center gap-3 px-4 py-2 hover:bg-slate-700/30">
      <div
        className={`w-8 h-8 rounded-full ${avatarColor} flex items-center justify-center text-sm font-bold shrink-0`}
      >
        {initials}
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium truncate">{displayName}</div>
        <div className="text-xs text-slate-400">{member.role}</div>
      </div>
      <button
        onClick={() => onRemove(member.user_id)}
        className="text-slate-400 hover:text-red-400 transition cursor-pointer p-1"
        title="Remove from channel"
      >
        <UserMinus className="w-4 h-4" />
      </button>
    </div>
  );
}

function AddableUserRow({ userId, onAdd }: { userId: string; onAdd: (id: string) => void }) {
  const { getUser } = useUserCache();
  const info = getUser(userId);
  const displayName = info?.display_name || userId.slice(0, 8);
  const initials = displayName.charAt(0).toUpperCase();
  const avatarColor = AVATAR_COLORS[getColorIndex(userId)];

  return (
    <button
      onClick={() => onAdd(userId)}
      className="w-full flex items-center gap-3 px-4 py-2 hover:bg-slate-700/30 cursor-pointer text-left"
    >
      <div
        className={`w-7 h-7 rounded-full ${avatarColor} flex items-center justify-center text-xs font-bold shrink-0`}
      >
        {initials}
      </div>
      <span className="text-sm truncate">{displayName}</span>
      <UserPlus className="w-3.5 h-3.5 text-slate-400 ml-auto shrink-0" />
    </button>
  );
}

export default function ChannelMembersPanel({ channelId, channelName, onClose }: Props) {
  const { users } = useUserCache();
  const { data: members = [], isLoading: loading } = useChannelMembers(channelId);
  const [showAddUser, setShowAddUser] = useState(false);
  const queryClient = useQueryClient();
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);
  const apiClient = currentWorkspace?.instanceUrl
    ? instanceManager.get(currentWorkspace.instanceUrl).api
    : api;

  const addMemberMutation = useMutation({
    mutationFn: async (userId: string) => {
      return apiClient.post<ChannelMember>(`/channels/${channelId}/members`, { user_id: userId });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelMembers(channelId) });
    },
  });

  const removeMemberMutation = useMutation({
    mutationFn: async (userId: string) => {
      return apiClient.delete(`/channels/${channelId}/members/${userId}`);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelMembers(channelId) });
    },
  });

  const channelMemberIds = new Set(members.map((m) => m.user_id));
  const nonMembers = Array.from(users.keys()).filter((id) => !channelMemberIds.has(id));

  return (
    <div className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-slate-800 border-l border-slate-700/50 flex flex-col">
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
              <button
                onClick={() => setShowAddUser(!showAddUser)}
                className="text-slate-400 hover:text-white transition cursor-pointer"
                title="Add member"
              >
                <UserPlus className="w-4 h-4" />
              </button>
            </div>
            {members.map((m) => (
              <MemberRow key={m.user_id} member={m} onRemove={(id) => removeMemberMutation.mutate(id)} />
            ))}
          </>
        )}
      </div>

      {showAddUser && nonMembers.length > 0 && (
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
