import { useState } from 'react';
import { Link2, Mail, X } from 'lucide-react';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import {
  isPendingEmailInvite,
  useCopyInviteLink,
  useRevokeInvite,
  useWorkspaceInvites,
  type WorkspaceInvite,
} from '@/features/workspace/hooks/useWorkspaces';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
}

export function PendingInvites({ workspaceId, instanceUrl }: Props) {
  const { data: invites = [] } = useWorkspaceInvites(workspaceId, instanceUrl);
  const copyLink = useCopyInviteLink(workspaceId, instanceUrl);
  const revoke = useRevokeInvite(workspaceId, instanceUrl);
  const [revoking, setRevoking] = useState<WorkspaceInvite | null>(null);

  const pending = invites.filter((invite) => isPendingEmailInvite(invite));
  if (pending.length === 0) return null;

  const onCopy = (invite: WorkspaceInvite) =>
    copyLink.mutate(invite.id, {
      onSuccess: () => toast.success(`Invite link for ${invite.email} copied`),
      onError: (err) => toast.error(toUserMessage(err, 'Failed to copy the invite link')),
    });

  return (
    <div className="pb-2" data-qa="pending-invites">
      <div className="px-4 mb-1">
        <span className="text-xs font-semibold text-muted uppercase tracking-wider">
          {pending.length} pending invite{pending.length !== 1 ? 's' : ''}
        </span>
      </div>
      {pending.map((invite) => (
        <div
          key={invite.id}
          data-qa="pending-invite-row"
          className="flex items-center gap-3 px-4 py-2 hover:bg-raised/30"
        >
          <Mail className="w-4 h-4 shrink-0 text-muted" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm text-fg-soft">{invite.email}</div>
            <div className="text-xs text-muted">
              {invite.role} · expires {new Date(invite.expires_at ?? '').toLocaleDateString()}
            </div>
          </div>
          <IconButton
            label={`Copy invite link for ${invite.email}`}
            icon={<Link2 className="w-4 h-4" />}
            onClick={() => onCopy(invite)}
            disabled={copyLink.isPending}
            data-qa="pending-invite-copy"
          />
          <IconButton
            label={`Revoke invite for ${invite.email}`}
            icon={<X className="w-4 h-4" />}
            tone="danger"
            onClick={() => setRevoking(invite)}
            data-qa="pending-invite-revoke"
          />
        </div>
      ))}
      {revoking && (
        <ConfirmDialog
          title={`Revoke the invite for ${revoking.email}?`}
          description="Its link stops working at once, including copies already sent."
          confirmLabel="Revoke invite"
          onConfirm={async () => {
            await revoke.mutateAsync(revoking.id);
          }}
          onClose={() => setRevoking(null)}
          dataQa="pending-invite-revoke-dialog"
        />
      )}
    </div>
  );
}
