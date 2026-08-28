import { useState, type FormEvent } from 'react';
import { api } from '../lib/api';
import type { WorkspaceMember } from '../stores/workspace';
import { useWorkspaceStore } from '../stores/workspace';
import { useCurrentWorkspaceRole } from '@/features/workspace/hooks/useCurrentWorkspaceRole';
import { getUserDisplay } from '../lib/userHelpers';
import { X, UserPlus, UserMinus, Crown, Shield, User, Mail } from 'lucide-react';
import PresenceDot from './PresenceDot';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useWorkspaceMembers } from '../hooks/queries/useWorkspaces';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { instanceManager } from '../lib/instances';
import { QUERY_KEYS } from '@/shared/constants';

interface Props {
  workspaceId: string;
  onClose: () => void;
}

const ROLE_LEVEL: Record<string, number> = { guest: 10, member: 20, channel_admin: 30, admin: 40, owner: 50 };
const ASSIGNABLE_ROLES = ['guest', 'member', 'admin', 'owner'] as const;

function MemberRow({
  member,
  actorRole,
  isSelf,
  onChangeRole,
  onRemove,
  busy,
}: {
  member: WorkspaceMember;
  actorRole: string | null;
  isSelf: boolean;
  onChangeRole: (userId: string, role: string) => void;
  onRemove: (userId: string) => void;
  busy: boolean;
}) {
  const { displayName, email } = getUserDisplay(member.user_id, [member]);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const actorLevel = actorRole ? (ROLE_LEVEL[actorRole] ?? 0) : 0;
  const targetLevel = ROLE_LEVEL[member.role] ?? 0;
  const canManage =
    !isSelf && actorLevel >= ROLE_LEVEL.admin && actorLevel > targetLevel && member.role !== 'owner';
  const grantable = ASSIGNABLE_ROLES.filter((r) => ROLE_LEVEL[r] <= actorLevel);

  const roleIcon = () => {
    switch (member.role) {
      case 'owner':
        return <Crown className="w-3.5 h-3.5 text-warning" />;
      case 'admin':
        return <Shield className="w-3.5 h-3.5 text-info" />;
      default:
        return <User className="w-3.5 h-3.5 text-muted" />;
    }
  };

  const roleName = member.role.charAt(0).toUpperCase() + member.role.slice(1);

  return (
    <div className="flex items-center gap-3 px-4 py-2.5 hover:bg-raised/30 rounded-lg">
      <div className="relative">
        <Avatar userId={member.user_id} name={displayName} avatarUrl={member.avatar_url} />
        <PresenceDot
          userId={member.user_id}
          className="absolute -bottom-0.5 -right-0.5 ring-2 ring-surface"
        />
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-fg-soft truncate">
          {displayName}
          {member.status_emoji && (
            <span className="ml-1.5" data-qa="member-status-emoji" title={member.status_text ?? undefined}>
              {member.status_emoji}
            </span>
          )}
        </div>
        {member.status_text ? (
          <div className="text-xs text-muted truncate" data-qa="member-status-text">
            {member.status_text}
          </div>
        ) : (
          email && <div className="text-xs text-muted truncate">{email}</div>
        )}
        {confirmRemove && (
          <div className="mt-1 flex items-center gap-2 text-xs">
            <span className="text-danger">Remove from workspace?</span>
            <button
              onClick={() => {
                onRemove(member.user_id);
                setConfirmRemove(false);
              }}
              data-qa="member-remove-confirm"
              className="px-2 py-0.5 bg-red-600 hover:bg-red-500 text-white rounded transition cursor-pointer"
            >
              Remove
            </button>
            <button
              onClick={() => setConfirmRemove(false)}
              className="text-muted hover:text-fg transition cursor-pointer"
            >
              Cancel
            </button>
          </div>
        )}
      </div>
      {canManage ? (
        <div className="flex items-center gap-1.5 shrink-0">
          <select
            value={member.role}
            disabled={busy}
            onChange={(e) => onChangeRole(member.user_id, e.target.value)}
            aria-label={`Role for ${displayName}`}
            data-qa="member-role-select"
            className="bg-raised/50 border border-line-strong rounded-md px-1.5 py-1 text-xs text-fg-soft focus:outline-none focus:ring-2 focus:ring-purple-500 disabled:opacity-50 cursor-pointer"
          >
            {grantable.map((r) => (
              <option key={r} value={r}>
                {r.charAt(0).toUpperCase() + r.slice(1)}
              </option>
            ))}
          </select>
          <button
            onClick={() => setConfirmRemove(true)}
            disabled={busy}
            aria-label={`Remove ${displayName}`}
            data-qa="member-remove"
            className="p-1 text-muted hover:text-danger transition cursor-pointer disabled:opacity-50"
          >
            <UserMinus className="w-3.5 h-3.5" />
          </button>
        </div>
      ) : (
        <div className="flex items-center gap-1.5 text-xs text-muted">
          {roleIcon()}
          <span>{roleName}</span>
        </div>
      )}
    </div>
  );
}

export default function MembersPanel({ workspaceId, onClose }: Props) {
  const queryClient = useQueryClient();
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const { role: actorRole } = useCurrentWorkspaceRole();
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);
  const { data: members = [], isLoading: loading } = useWorkspaceMembers(workspaceId, instanceUrl);
  const [manageError, setManageError] = useState<string | null>(null);

  const apiClient = instanceUrl ? instanceManager.get(instanceUrl).api : api;
  const refreshMembers = () =>
    queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });

  const changeRole = useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: string }) =>
      apiClient.patch(`/workspaces/${workspaceId}/members/${userId}/role`, { role }),
    onSuccess: () => {
      setManageError(null);
      refreshMembers();
    },
    onError: (err: unknown) =>
      setManageError((err as { message?: string })?.message || 'Failed to change role'),
  });

  const removeMember = useMutation({
    mutationFn: (userId: string) => apiClient.delete(`/workspaces/${workspaceId}/members/${userId}`),
    onSuccess: () => {
      setManageError(null);
      refreshMembers();
    },
    onError: (err: unknown) =>
      setManageError((err as { message?: string })?.message || 'Failed to remove member'),
  });

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
    <div className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface/80 border-l border-line/50 flex flex-col h-full">
      <div className="h-14 px-4 flex items-center justify-between border-b border-line/50 shrink-0">
        <h2 className="font-semibold text-fg">Members</h2>
        <button onClick={onClose} className="text-muted hover:text-fg transition cursor-pointer">
          <X className="w-5 h-5" />
        </button>
      </div>

      <div className="px-4 py-3 border-b border-line/50 shrink-0">
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
              <Mail className="w-4 h-4 text-muted shrink-0" />
              <input
                type="email"
                value={inviteEmail}
                onChange={(e) => setInviteEmail(e.target.value)}
                placeholder="user@example.com"
                className="flex-1 px-3 py-2 bg-raised/50 border border-line-strong rounded-lg text-fg text-sm placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500"
                autoFocus
                required
              />
            </div>
            <select
              value={inviteRole}
              onChange={(e) => setInviteRole(e.target.value)}
              className="w-full px-3 py-2 bg-raised/50 border border-line-strong rounded-lg text-fg text-sm focus:outline-none focus:ring-2 focus:ring-purple-500"
            >
              <option value="member">Member</option>
              <option value="admin">Admin</option>
              <option value="guest">Guest</option>
            </select>
            {inviteError && <div className="text-xs text-danger px-1">{inviteError}</div>}
            {inviteResult && <div className="text-xs text-success px-1">{inviteResult}</div>}
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => {
                  setShowInvite(false);
                  setInviteError(null);
                  setInviteResult(null);
                }}
                className="flex-1 px-3 py-2 text-sm text-muted hover:text-fg transition cursor-pointer"
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
          <div className="text-center text-muted text-sm py-8">No members found</div>
        ) : (
          <>
            <div className="px-4 mb-1">
              <span className="text-xs font-semibold text-muted uppercase tracking-wider">
                {members.length} member{members.length !== 1 ? 's' : ''}
              </span>
            </div>
            {manageError && <div className="px-4 pb-1 text-xs text-danger">{manageError}</div>}
            {members.map((m) => (
              <MemberRow
                key={m.user_id}
                member={m}
                actorRole={actorRole}
                isSelf={m.user_id === currentUserId}
                busy={changeRole.isPending || removeMember.isPending}
                onChangeRole={(userId, role) => changeRole.mutate({ userId, role })}
                onRemove={(userId) => removeMember.mutate(userId)}
              />
            ))}
          </>
        )}
      </div>
    </div>
  );
}
