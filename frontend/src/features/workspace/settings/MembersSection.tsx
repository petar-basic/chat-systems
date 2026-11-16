import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { toUserMessage } from '@/lib/errors';
import type { WorkspaceMember, WorkspaceRole } from '@/stores/workspace';
import { useWorkspaceStore } from '@/stores/workspace';
import { useCurrentWorkspaceRole } from '@/features/workspace/hooks/useCurrentWorkspaceRole';
import { getUserDisplay } from '@/lib/userHelpers';
import { UserPlus, UserMinus, Crown, Shield, User, PackageOpen, Eraser, CalendarClock } from 'lucide-react';
import PresenceDot from '@/shared/components/PresenceDot';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useWorkspaceMembers } from '@/features/workspace/hooks/useWorkspaces';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { QUERY_KEYS } from '@/shared/constants';
import { Select } from '@/shared/components/Form/Select';
import { useErasePerson, useStartPersonExport } from '@/features/workspace/hooks/useExports';
import { ErasePersonDialog } from './ErasePersonDialog';
import { ReactionEmoji } from '@/shared/components/ReactionEmoji';
import { PendingInvites } from './PendingInvites';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { formatDay } from '@/lib/datetime';
import { GuestSeatDialog } from './GuestSeatDialog';
import { endingSoon } from './guestTerms';
import { allows, ruleOf } from '@/lib/workspaceRules';
import { DirectInviteForm } from './DirectInviteForm';
import { InviteRequestForm } from './InviteRequestForm';
import { InviteRequests } from './InviteRequests';
import { MyInviteRequests } from './MyInviteRequests';

interface Props {
  workspaceId: string;
  openInvite: boolean;
  isInstanceAdmin: boolean;
}

const ROLE_LEVEL: Record<string, number> = { guest: 10, member: 20, channel_admin: 30, admin: 40, owner: 50 };
const ASSIGNABLE_ROLES = ['guest', 'member', 'admin', 'owner'] as const;
const INVITABLE_ROLES: WorkspaceRole[] = ['member', 'admin', 'guest'];

function GuestTerms({ member }: { member: WorkspaceMember }) {
  const { t } = useTranslation('workspace');
  if (member.role !== 'guest' || !member.guest_kind) return null;
  const kind = member.guest_kind === 'single' ? t('guest.single') : t('guest.multi');
  return (
    <div className="text-xs text-muted truncate" data-qa="member-guest-terms">
      {member.guest_until ? t('guest.kindUntil', { kind, date: formatDay(member.guest_until) }) : kind}
    </div>
  );
}

function MemberRow({
  workspaceId,
  instanceUrl,
  member,
  actorRole,
  isSelf,
  canStepDown,
  canHandleData,
  onChangeRole,
  onRemove,
  onExport,
  onErase,
  busy,
}: {
  workspaceId: string;
  instanceUrl?: string;
  member: WorkspaceMember;
  actorRole: string | null;
  isSelf: boolean;
  canStepDown: boolean;
  canHandleData: boolean;
  onChangeRole: (userId: string, role: WorkspaceRole) => void;
  onRemove: (userId: string) => void;
  onExport: (userId: string) => void;
  onErase: (userId: string) => Promise<unknown>;
  busy: boolean;
}) {
  const { displayName, email } = getUserDisplay(member.user_id, [member]);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [confirmErase, setConfirmErase] = useState(false);
  const [confirmStepDown, setConfirmStepDown] = useState(false);
  const [editGuest, setEditGuest] = useState(false);
  const { t } = useTranslation('workspace');

  const actorLevel = actorRole ? (ROLE_LEVEL[actorRole] ?? 0) : 0;
  const targetLevel = ROLE_LEVEL[member.role] ?? 0;
  const canManage =
    !isSelf &&
    !member.is_bot &&
    actorLevel >= ROLE_LEVEL.admin &&
    actorLevel > targetLevel &&
    member.role !== 'owner';
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
    <div className="flex items-center gap-3 px-4 py-2.5 hover:bg-raised/30 rounded-lg" data-qa="member-row">
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
          {member.is_bot && (
            <span
              data-qa="member-app-tag"
              className="ml-1.5 px-1 py-0.5 text-3xs font-semibold uppercase tracking-wide bg-raised text-fg-dim rounded"
            >
              App
            </span>
          )}
          {member.status_emoji && (
            <span
              className="ml-1.5"
              data-qa="member-status-emoji"
              data-tooltip={member.status_text ?? undefined}
            >
              <ReactionEmoji emoji={member.status_emoji} />
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
        <GuestTerms member={member} />
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
        {confirmStepDown && (
          <div className="mt-1 flex items-center gap-2 text-xs">
            <span className="text-fg-soft">
              Step down to admin? Only an owner can make you an owner again.
            </span>
            <Button
              variant="primary"
              size="sm"
              onClick={() => {
                onChangeRole(member.user_id, 'admin');
                setConfirmStepDown(false);
              }}
              data-qa="member-step-down-confirm"
            >
              Step down
            </Button>
            <button
              onClick={() => setConfirmStepDown(false)}
              className="text-muted hover:text-fg transition cursor-pointer"
            >
              Cancel
            </button>
          </div>
        )}
      </div>
      {canHandleData && !isSelf && !member.is_bot && (
        <div className="flex items-center gap-0.5 shrink-0">
          <button
            onClick={() => onExport(member.user_id)}
            disabled={busy}
            aria-label={`Export ${displayName}'s data`}
            data-tooltip="Export their data"
            data-qa="member-export"
            className="p-1 text-muted hover:text-fg transition cursor-pointer disabled:opacity-50"
          >
            <PackageOpen className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={() => setConfirmErase(true)}
            disabled={busy}
            aria-label={`Erase ${displayName}'s data`}
            data-tooltip="Erase their data"
            data-qa="member-erase"
            className="p-1 text-muted hover:text-danger transition cursor-pointer disabled:opacity-50"
          >
            <Eraser className="w-3.5 h-3.5" />
          </button>
        </div>
      )}
      {confirmErase && (
        <ErasePersonDialog
          name={displayName}
          email={email}
          onErase={() => onErase(member.user_id)}
          onClose={() => setConfirmErase(false)}
        />
      )}
      {editGuest && (
        <GuestSeatDialog
          workspaceId={workspaceId}
          instanceUrl={instanceUrl}
          member={member}
          name={displayName}
          onClose={() => setEditGuest(false)}
        />
      )}
      {canManage ? (
        <div className="flex items-center gap-1.5 shrink-0">
          {member.role === 'guest' && (
            <button
              onClick={() => setEditGuest(true)}
              disabled={busy}
              aria-label={t('guest.editTitle', { name: displayName })}
              data-tooltip={t('guest.edit')}
              data-qa="member-guest-edit"
              className="p-1 text-muted hover:text-fg transition cursor-pointer disabled:opacity-50"
            >
              <CalendarClock className="w-3.5 h-3.5" />
            </button>
          )}
          <Select
            label={`Role for ${displayName}`}
            labelHidden
            compact
            value={member.role}
            disabled={busy}
            onChange={(e) => {
              const role = grantable.find((r) => r === e.target.value);
              if (role) onChangeRole(member.user_id, role);
            }}
            dataQa="member-role-select"
            className="w-28"
          >
            {grantable.map((r) => (
              <option key={r} value={r}>
                {r.charAt(0).toUpperCase() + r.slice(1)}
              </option>
            ))}
          </Select>
          <button
            onClick={() => setConfirmRemove(true)}
            disabled={busy}
            aria-label={`Remove ${displayName}`}
            data-tooltip={`Remove ${displayName}`}
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
          {isSelf && canStepDown && !confirmStepDown && (
            <button
              onClick={() => setConfirmStepDown(true)}
              disabled={busy}
              data-qa="member-step-down"
              className="ml-1 px-2 py-0.5 rounded text-xs text-muted hover:text-fg hover:bg-raised transition cursor-pointer disabled:opacity-50"
            >
              Step down
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export function MembersSection({ workspaceId, openInvite, isInstanceAdmin }: Props) {
  const { t } = useTranslation('workspace');
  const queryClient = useQueryClient();
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const { role: actorRole } = useCurrentWorkspaceRole();
  const currentUserId = useWorkspaceStore((s) => s.currentUserId);
  const workspaceSettings = useWorkspaceStore((s) => s.currentWorkspace?.settings);
  const { data: members = [], isLoading: loading } = useWorkspaceMembers(workspaceId, instanceUrl);
  const [manageError, setManageError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const startExport = useStartPersonExport(workspaceId, instanceUrl);
  const erase = useErasePerson(workspaceId, instanceUrl);

  const apiClient = getApiForInstance(instanceUrl);
  const refreshMembers = () =>
    queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });

  const changeRole = useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: WorkspaceRole }) =>
      apiClient.typed((c) =>
        c.PATCH('/workspaces/{ws_id}/members/{user_id}/role', {
          params: { path: { ws_id: workspaceId, user_id: userId } },
          body: { role },
        }),
      ),
    onSuccess: () => {
      setManageError(null);
      refreshMembers();
    },
    onError: (err: unknown) => setManageError(toUserMessage(err, 'Failed to change role')),
  });

  const removeMember = useMutation({
    mutationFn: (userId: string) =>
      apiClient.typed((c) =>
        c.DELETE('/workspaces/{ws_id}/members/{user_id}', {
          params: { path: { ws_id: workspaceId, user_id: userId } },
        }),
      ),
    onSuccess: () => {
      setManageError(null);
      refreshMembers();
    },
    onError: (err: unknown) => setManageError(toUserMessage(err, 'Failed to remove member')),
  });

  const isAdmin = actorRole === 'admin' || actorRole === 'owner';
  const otherOwners = members.filter(
    (m) => m.role === 'owner' && !m.is_bot && m.user_id !== currentUserId,
  ).length;
  const [showInvite, setShowInvite] = useState(openInvite);
  const [onlyEnding, setOnlyEnding] = useState(false);
  const shown = onlyEnding ? members.filter((m) => endingSoon(m.guest_until)) : members;
  const isMember = !!actorRole && actorRole !== 'guest';
  const invitesDirectly = allows(ruleOf(workspaceSettings, 'inviters'), actorRole);
  const actorLevel = actorRole ? (ROLE_LEVEL[actorRole] ?? 0) : 0;
  const invitable = INVITABLE_ROLES.filter((r) => ROLE_LEVEL[r] <= actorLevel);

  return (
    <div className="-mx-4 flex flex-col">
      {isMember && (
        <div className="px-4 py-3 border-b border-line/50 shrink-0">
          {!showInvite ? (
            <Button variant="primary" fullWidth onClick={() => setShowInvite(true)} data-qa="members-invite">
              <UserPlus className="w-4 h-4" />
              Invite people
            </Button>
          ) : invitesDirectly ? (
            <DirectInviteForm
              workspaceId={workspaceId}
              instanceUrl={instanceUrl}
              roles={invitable}
              onClose={() => setShowInvite(false)}
            />
          ) : (
            <InviteRequestForm
              workspaceId={workspaceId}
              instanceUrl={instanceUrl}
              onClose={() => setShowInvite(false)}
            />
          )}
        </div>
      )}

      <div className="flex-1 overflow-y-auto py-2">
        {isAdmin && <InviteRequests workspaceId={workspaceId} instanceUrl={instanceUrl} />}
        {isMember && !isAdmin && <MyInviteRequests workspaceId={workspaceId} instanceUrl={instanceUrl} />}
        {isAdmin && <PendingInvites workspaceId={workspaceId} instanceUrl={instanceUrl} />}
        {loading ? (
          <div className="flex justify-center py-8">
            <Spinner className="text-accent" />
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
            {isAdmin && members.some((m) => m.guest_until) && (
              <div className="px-4 pb-2">
                <Checkbox
                  label={t('guest.endingSoon')}
                  checked={onlyEnding}
                  onChange={setOnlyEnding}
                  dataQa="members-filter-ending"
                />
              </div>
            )}
            {onlyEnding && shown.length === 0 && (
              <div className="px-4 pb-2 text-xs text-muted" data-qa="members-none-ending">
                {t('guest.noneEnding')}
              </div>
            )}
            {actorRole === 'owner' && (
              <div className="px-4 pb-2 text-xs text-muted" data-qa="owner-note">
                {otherOwners > 0
                  ? 'Another owner remains, so you can step down to admin or leave the workspace.'
                  : "You're the only owner. To step down or leave, first make someone else an owner: pick Owner in their row."}
              </div>
            )}
            {manageError && <div className="px-4 pb-1 text-xs text-danger">{manageError}</div>}
            {notice && (
              <div className="px-4 pb-1 text-xs text-success" data-qa="member-data-notice">
                {notice}
              </div>
            )}
            {shown.map((m) => (
              <MemberRow
                key={m.user_id}
                workspaceId={workspaceId}
                instanceUrl={instanceUrl}
                member={m}
                actorRole={actorRole}
                isSelf={m.user_id === currentUserId}
                canStepDown={m.role === 'owner' && otherOwners > 0}
                canHandleData={isInstanceAdmin}
                busy={changeRole.isPending || removeMember.isPending || startExport.isPending}
                onChangeRole={(userId, role) => changeRole.mutate({ userId, role })}
                onRemove={(userId) => removeMember.mutate(userId)}
                onExport={(userId) =>
                  startExport.mutate(userId, {
                    onSuccess: () => {
                      setManageError(null);
                      setNotice(
                        `Export started for ${getUserDisplay(userId, members).displayName}. It appears under Exports.`,
                      );
                    },
                    onError: (err) => setManageError(toUserMessage(err, 'Failed to start the export')),
                  })
                }
                onErase={async (userId) => {
                  await erase.mutateAsync(userId);
                  setNotice(`${getUserDisplay(userId, members).displayName}'s data was erased.`);
                }}
              />
            ))}
          </>
        )}
      </div>
    </div>
  );
}
