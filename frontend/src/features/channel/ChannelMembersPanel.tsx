import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { UserPlus, UserMinus, Hash, ShieldCheck, Shield } from 'lucide-react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useUserCache } from '@/stores/users';
import { useWorkspaceStore } from '@/stores/workspace';
import { useChannelArchived } from '@/lib/channelArchive';
import { useUpdateChannelMemberRole } from './hooks/useChannels';
import { useChannelModeration } from '@/features/channel/hooks/useChannelModeration';
import { canAddChannelMembers, type ChannelRole } from '@/lib/channelPermissions';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { QUERY_KEYS } from '@/shared/constants';
import type { components } from '@/api/schema';

type ChannelMember = components['schemas']['ChannelMember'];

interface Props {
  channelId: string;
  channelName: string;
  channelType: string;
  startAdding?: boolean;
  onClose: () => void;
}

function MemberRow({
  member,
  isSelf,
  canModerate,
  locked,
  busy,
  onSetRole,
  onRemove,
}: {
  member: ChannelMember;
  isSelf: boolean;
  canModerate: boolean;
  locked: boolean;
  busy: boolean;
  onSetRole: (userId: string, role: ChannelRole) => void;
  onRemove: (userId: string) => void;
}) {
  const { t } = useTranslation('channel');
  const { getUser } = useUserCache();
  const info = getUser(member.user_id);
  const displayName = info?.display_name || member.user_id.slice(0, 8);
  const isAdmin = member.role === 'admin';

  return (
    <div
      className="flex items-center gap-3 px-4 py-2 hover:bg-raised/30"
      data-qa="channel-member-row"
      data-user-id={member.user_id}
    >
      <Avatar userId={member.user_id} name={displayName} avatarUrl={info?.avatar_url} />
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium truncate">
          {displayName}
          {isSelf && <span className="text-muted font-normal">{t('members.you')}</span>}
        </div>
        <div className="text-xs text-muted flex items-center gap-1">
          {isAdmin && <ShieldCheck className="w-3 h-3 text-info" />}
          <span data-qa="channel-member-role">
            {isAdmin ? t('members.roleAdmin') : t('members.roleMember')}
          </span>
        </div>
      </div>
      {canModerate && !locked && (
        <button
          onClick={() => onSetRole(member.user_id, isAdmin ? 'member' : 'admin')}
          disabled={busy}
          aria-label={t(isAdmin ? 'members.demoteNamed' : 'members.promoteNamed', { name: displayName })}
          data-tooltip={isAdmin ? t('members.demote') : t('members.promote')}
          data-qa={isAdmin ? 'channel-member-demote' : 'channel-member-promote'}
          className={`p-1 rounded transition cursor-pointer disabled:opacity-50 ${
            isAdmin ? 'text-info hover:text-fg-dim' : 'text-muted hover:text-info'
          }`}
        >
          <Shield className="w-4 h-4" />
        </button>
      )}
      {!locked && (canModerate || isSelf) && (
        <button
          onClick={() => onRemove(member.user_id)}
          disabled={busy}
          aria-label={isSelf ? t('members.leave') : t('members.removeNamed', { name: displayName })}
          data-tooltip={isSelf ? t('members.leave') : t('members.remove')}
          data-qa="channel-member-remove"
          className="text-muted hover:text-danger transition cursor-pointer p-1 disabled:opacity-50"
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
      className="w-full flex items-center gap-3 px-4 py-2 hover:bg-raised/30 cursor-pointer text-left"
    >
      <Avatar userId={userId} name={displayName} avatarUrl={info?.avatar_url} size="sm" />
      <span className="text-sm truncate">{displayName}</span>
      <UserPlus className="w-3.5 h-3.5 text-muted ml-auto shrink-0" />
    </button>
  );
}

export default function ChannelMembersPanel({
  channelId,
  channelName,
  channelType,
  startAdding = false,
  onClose,
}: Props) {
  const { t } = useTranslation('channel');
  const { users } = useUserCache();
  const {
    members,
    isLoading: loading,
    canModerate,
    myRole,
    workspaceRole,
    currentUserId,
  } = useChannelModeration(channelId);
  const [showAddUser, setShowAddUser] = useState(startAdding);
  const [error, setError] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);
  const apiClient = currentWorkspace?.instanceUrl ? getApiForInstance(currentWorkspace.instanceUrl) : api;

  const archived = useChannelArchived(channelId);
  const canAdd = !archived && canAddChannelMembers(workspaceRole, myRole, channelType);

  const failWith = (fallback: string) => (err: unknown) =>
    setError((err as { message?: string })?.message || fallback);

  const addMemberMutation = useMutation({
    mutationFn: async (userId: string) => {
      return apiClient.typed((c) =>
        c.POST('/channels/{ch_id}/members', {
          params: { path: { ch_id: channelId } },
          body: { user_id: userId },
        }),
      );
    },
    onSuccess: () => {
      setError(null);
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelMembers(channelId) });
    },
    onError: failWith(t('members.addFailed')),
  });

  const removeMemberMutation = useMutation({
    mutationFn: async (userId: string) => {
      return apiClient.typed((c) =>
        c.DELETE('/channels/{ch_id}/members/{user_id}', {
          params: { path: { ch_id: channelId, user_id: userId } },
        }),
      );
    },
    onSuccess: () => {
      setError(null);
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.channelMembers(channelId) });
    },
    onError: failWith(t('members.removeFailed')),
  });

  const roleMutation = useUpdateChannelMemberRole(channelId);

  const channelMemberIds = new Set(members.map((m) => m.user_id));
  const nonMembers = Array.from(users.keys()).filter((id) => !channelMemberIds.has(id));
  const busy = addMemberMutation.isPending || removeMemberMutation.isPending || roleMutation.isPending;

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="channel-members-panel"
    >
      <PanelHeader
        title={t('members.title', { name: channelName })}
        icon={<Hash className="h-4 w-4 text-muted" />}
        closeLabel={t('members.close')}
        onClose={onClose}
      />

      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex justify-center py-8">
            <Spinner className="text-accent" />
          </div>
        ) : (
          <>
            <div className="px-4 py-2 flex items-center justify-between">
              <span className="text-xs font-semibold text-muted uppercase tracking-wider">
                {t('members.count', { total: members.length })}
              </span>
              {canAdd && (
                <button
                  onClick={() => setShowAddUser(!showAddUser)}
                  className="text-muted hover:text-fg transition cursor-pointer"
                  aria-label={t('members.add')}
                  data-qa="channel-members-add-toggle"
                  data-tooltip={t('members.add')}
                >
                  <UserPlus className="w-4 h-4" />
                </button>
              )}
            </div>
            {error && <div className="px-4 pb-1 text-xs text-danger">{error}</div>}
            {members.map((m) => (
              <MemberRow
                key={m.user_id}
                member={m}
                isSelf={m.user_id === currentUserId}
                canModerate={canModerate}
                locked={archived}
                busy={busy}
                onSetRole={(userId, role) => roleMutation.mutate({ userId, role })}
                onRemove={(id) => removeMemberMutation.mutate(id)}
              />
            ))}
          </>
        )}
      </div>

      {showAddUser && canAdd && nonMembers.length > 0 && (
        <div className="border-t border-line/50 max-h-48 overflow-y-auto">
          <div className="px-4 py-2 text-xs font-semibold text-muted uppercase tracking-wider">
            {t('members.addTo')}
          </div>
          {nonMembers.map((userId) => (
            <AddableUserRow key={userId} userId={userId} onAdd={(id) => addMemberMutation.mutate(id)} />
          ))}
        </div>
      )}
      {showAddUser && canAdd && !loading && nonMembers.length === 0 && (
        <p
          className="border-t border-line/50 px-4 py-3 text-xs text-muted"
          data-qa="channel-members-none-to-add"
        >
          {t('members.everyoneIn')}
        </p>
      )}
    </div>
  );
}
