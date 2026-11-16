import { useState, type FormEvent } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Mail } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { CopyField } from '@/shared/components/CopyField/CopyField';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { toUserMessage } from '@/lib/errors';
import type { WorkspaceRole } from '@/stores/workspace';
import { inviteMessage } from './inviteMessage';
import { GuestInviteFields } from './GuestInviteFields';
import { endOfDay, NO_GUEST, type GuestInvite } from './guestTerms';

const ROLE_LABEL: Partial<Record<WorkspaceRole, string>> = {
  member: 'Member',
  admin: 'Admin',
  guest: 'Guest',
};

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  roles: WorkspaceRole[];
  onClose: () => void;
}

export function DirectInviteForm({ workspaceId, instanceUrl, roles, onClose }: Props) {
  const queryClient = useQueryClient();
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState<WorkspaceRole>('member');
  const [guestInvite, setGuestInvite] = useState<GuestInvite>(NO_GUEST);
  const [inviting, setInviting] = useState(false);
  const [inviteResult, setInviteResult] = useState<{ message: string; url?: string | null } | null>(null);
  const [inviteError, setInviteError] = useState<string | null>(null);
  const inviteIsGuest = inviteRole === 'guest';

  const handleInvite = async (e: FormEvent) => {
    e.preventDefault();
    if (!inviteEmail.trim()) return;

    setInviting(true);
    setInviteError(null);
    setInviteResult(null);
    try {
      const email = inviteEmail.trim();
      const created = await getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/invites', {
          params: { path: { ws_id: workspaceId } },
          body: inviteIsGuest
            ? {
                email,
                role: inviteRole,
                guest_kind: guestInvite.kind,
                channel_ids: guestInvite.channelIds,
                guest_until: guestInvite.until ? endOfDay(guestInvite.until) : null,
              }
            : { email, role: inviteRole },
        }),
      );
      setInviteResult({
        message: inviteMessage(created.email_status, email),
        url: created.invite_url,
      });
      setInviteEmail('');
      setGuestInvite(NO_GUEST);
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceInvites(workspaceId) });
    } catch (err: unknown) {
      setInviteError(toUserMessage(err, 'Failed to invite user'));
    } finally {
      setInviting(false);
    }
  };

  return (
    <form onSubmit={handleInvite} className="space-y-2">
      <TextInput
        label="Email address to invite"
        labelHidden
        type="email"
        leading={<Mail className="w-4 h-4" />}
        value={inviteEmail}
        onChange={(e) => setInviteEmail(e.target.value)}
        placeholder="user@example.com"
        autoFocus
        required
      />
      <Select
        label="Invite role"
        labelHidden
        value={inviteRole}
        onChange={(e) => {
          const role = roles.find((r) => r === e.target.value);
          if (role) setInviteRole(role);
        }}
        dataQa="invite-role"
      >
        {roles.map((r) => (
          <option key={r} value={r}>
            {ROLE_LABEL[r] ?? r}
          </option>
        ))}
      </Select>
      {inviteIsGuest && (
        <GuestInviteFields
          workspaceId={workspaceId}
          instanceUrl={instanceUrl}
          value={guestInvite}
          onChange={setGuestInvite}
        />
      )}
      {inviteError && <div className="text-xs text-danger px-1">{inviteError}</div>}
      {inviteResult && (
        <div className="px-1" data-qa="invite-result">
          <div className="text-xs text-fg-soft">{inviteResult.message}</div>
          {inviteResult.url && (
            <CopyField
              label="Invite link"
              value={inviteResult.url}
              valueQa="invite-link-value"
              copyQa="invite-link-copy"
            />
          )}
        </div>
      )}
      <div className="flex gap-2">
        <Button variant="ghost" className="flex-1" onClick={onClose}>
          Cancel
        </Button>
        <Button
          variant="primary"
          className="flex-1"
          type="submit"
          disabled={inviting || !inviteEmail.trim() || (inviteIsGuest && guestInvite.channelIds.length === 0)}
          data-qa="invite-submit"
        >
          {inviting ? 'Inviting…' : 'Invite'}
        </Button>
      </div>
    </form>
  );
}
