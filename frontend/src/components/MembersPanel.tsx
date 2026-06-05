import { useState, type FormEvent } from 'react';
import { api } from '../lib/api';
import type { WorkspaceMember } from '../stores/workspace';
import { useWorkspaceStore } from '../stores/workspace';
import { getUserDisplay } from '../lib/userHelpers';
import { X, UserPlus, Crown, Shield, User, Mail } from 'lucide-react';
import PresenceDot from './PresenceDot';
import { useWorkspaceMembers } from '../hooks/queries/useWorkspaces';
import { useQueryClient } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';

interface Props {
  workspaceId: string;
  onClose: () => void;
}

function MemberRow({ member }: { member: WorkspaceMember }) {
  const { displayName, email } = getUserDisplay(member.user_id, [member]);

  const roleIcon = () => {
    switch (member.role) {
      case 'owner':
        return <Crown className="w-3.5 h-3.5 text-amber-400" />;
      case 'admin':
        return <Shield className="w-3.5 h-3.5 text-blue-400" />;
      default:
        return <User className="w-3.5 h-3.5 text-slate-400" />;
    }
  };

  const roleName = member.role.charAt(0).toUpperCase() + member.role.slice(1);

  return (
    <div className="flex items-center gap-3 px-4 py-2.5 hover:bg-slate-700/30 rounded-lg">
      <div className="relative">
        <div className="w-8 h-8 rounded-full bg-purple-600 flex items-center justify-center text-sm font-bold shrink-0">
          {displayName.charAt(0).toUpperCase()}
        </div>
        <PresenceDot
          userId={member.user_id}
          className="absolute -bottom-0.5 -right-0.5 ring-2 ring-slate-800"
        />
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-slate-200 truncate">{displayName}</div>
        {email && <div className="text-xs text-slate-400 truncate">{email}</div>}
      </div>
      <div className="flex items-center gap-1.5 text-xs text-slate-400">
        {roleIcon()}
        <span>{roleName}</span>
      </div>
    </div>
  );
}

export default function MembersPanel({ workspaceId, onClose }: Props) {
  const queryClient = useQueryClient();
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const { data: members = [], isLoading: loading } = useWorkspaceMembers(workspaceId, instanceUrl);

  const [showInvite, setShowInvite] = useState(false);
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState('member');
  const [inviting, setInviting] = useState(false);
  const [inviteResult, setInviteResult] = useState<string | null>(null);
  const [inviteError, setInviteError] = useState<string | null>(null);

  const handleInvite = async (e: FormEvent) => {
    e.preventDefault();
    if (!inviteEmail.trim()) return;

    setInviting(true);
    setInviteError(null);
    setInviteResult(null);
    try {
      const res = await api.post<{ action: string }>(`/workspaces/${workspaceId}/invites`, {
        email: inviteEmail.trim(),
        role: inviteRole,
      });
      if (res.action === 'added_directly') {
        setInviteResult(`${inviteEmail} has been added to the workspace.`);
      } else {
        setInviteResult(`Invite sent to ${inviteEmail}.`);
      }
      setInviteEmail('');
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });
    } catch (err: unknown) {
      const msg = (err as { message?: string })?.message || 'Failed to invite user';
      setInviteError(msg);
    } finally {
      setInviting(false);
    }
  };

  return (
    <div className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-slate-800/80 border-l border-slate-700/50 flex flex-col h-full">
      <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700/50 shrink-0">
        <h2 className="font-semibold text-white">Members</h2>
        <button onClick={onClose} className="text-slate-400 hover:text-white transition cursor-pointer">
          <X className="w-5 h-5" />
        </button>
      </div>

      <div className="px-4 py-3 border-b border-slate-700/50 shrink-0">
        {!showInvite ? (
          <button
            onClick={() => setShowInvite(true)}
            className="w-full flex items-center justify-center gap-2 px-3 py-2 bg-purple-600 hover:bg-purple-500 text-white text-sm font-medium rounded-lg transition cursor-pointer"
          >
            <UserPlus className="w-4 h-4" />
            Invite People
          </button>
        ) : (
          <form onSubmit={handleInvite} className="space-y-2">
            <div className="flex items-center gap-2">
              <Mail className="w-4 h-4 text-slate-400 shrink-0" />
              <input
                type="email"
                value={inviteEmail}
                onChange={(e) => setInviteEmail(e.target.value)}
                placeholder="user@example.com"
                className="flex-1 px-3 py-2 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
                autoFocus
                required
              />
            </div>
            <select
              value={inviteRole}
              onChange={(e) => setInviteRole(e.target.value)}
              className="w-full px-3 py-2 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm focus:outline-none focus:ring-2 focus:ring-purple-500"
            >
              <option value="member">Member</option>
              <option value="admin">Admin</option>
              <option value="guest">Guest</option>
            </select>
            {inviteError && <div className="text-xs text-red-400 px-1">{inviteError}</div>}
            {inviteResult && <div className="text-xs text-green-400 px-1">{inviteResult}</div>}
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => {
                  setShowInvite(false);
                  setInviteError(null);
                  setInviteResult(null);
                }}
                className="flex-1 px-3 py-2 text-sm text-slate-400 hover:text-white transition cursor-pointer"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={inviting || !inviteEmail.trim()}
                className="flex-1 px-3 py-2 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white text-sm font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
              >
                {inviting ? 'Sending...' : 'Send Invite'}
              </button>
            </div>
          </form>
        )}
      </div>

      <div className="flex-1 overflow-y-auto py-2">
        {loading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : members.length === 0 ? (
          <div className="text-center text-slate-400 text-sm py-8">No members found</div>
        ) : (
          <>
            <div className="px-4 mb-1">
              <span className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
                {members.length} member{members.length !== 1 ? 's' : ''}
              </span>
            </div>
            {members.map((m) => (
              <MemberRow key={m.user_id} member={m} />
            ))}
          </>
        )}
      </div>
    </div>
  );
}
