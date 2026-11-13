import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { UserPlus } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { TextArea } from '@/shared/components/Form/TextArea';
import { toast } from '@/shared/components/Toast';
import { formatDay } from '@/lib/datetime';
import { toUserMessage } from '@/lib/errors';
import {
  type InviteRequest,
  useApproveInviteRequest,
  useDeclineInviteRequest,
  useInviteRequests,
} from '@/features/workspace/hooks/useInviteRequests';
import { InviteRequestDialog } from './InviteRequestDialog';
import { MAX_NOTE } from './inviteRequestTerms';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
}

function RequestTermsLine({ request }: { request: InviteRequest }) {
  const { t } = useTranslation('workspace');
  if (request.role !== 'guest') return <>{t('inviteRequest.asMember')}</>;
  const kind = request.guest_kind === 'single' ? t('guest.single') : t('guest.multi');
  const channels = request.channel_names.map((name) => `#${name}`).join(', ');
  const terms = request.guest_until
    ? t('guest.kindUntil', { kind, date: formatDay(request.guest_until) })
    : kind;
  return <>{channels ? `${terms} · ${channels}` : terms}</>;
}

export function InviteRequests({ workspaceId, instanceUrl }: Props) {
  const { t } = useTranslation('workspace');
  const { data: requests = [] } = useInviteRequests(workspaceId, instanceUrl);
  const approve = useApproveInviteRequest(workspaceId, instanceUrl);
  const decline = useDeclineInviteRequest(workspaceId, instanceUrl);
  const [editing, setEditing] = useState<InviteRequest | null>(null);
  const [declining, setDeclining] = useState<InviteRequest | null>(null);
  const [note, setNote] = useState('');

  if (requests.length === 0) return null;

  const onApprove = (request: InviteRequest) =>
    approve.mutate(
      { requestId: request.id },
      {
        onSuccess: (approved) =>
          toast.success(t('inviteRequest.approved', { count: approved.invites.length })),
        onError: (err) => toast.error(toUserMessage(err, t('inviteRequest.approveFailed'))),
      },
    );

  return (
    <div className="pb-2" data-qa="invite-requests">
      <div className="px-4 mb-1">
        <span className="text-xs font-semibold text-muted uppercase tracking-wider">
          {t('inviteRequest.title', { count: requests.length })}
        </span>
      </div>
      {requests.map((request) => (
        <div
          key={request.id}
          data-qa="invite-request-row"
          className="flex gap-3 px-4 py-2 hover:bg-raised/30"
        >
          <UserPlus className="w-4 h-4 mt-0.5 shrink-0 text-muted" />
          <div className="min-w-0 flex-1 space-y-0.5">
            <div className="text-sm text-fg-soft break-words" data-qa="invite-request-emails-text">
              {request.emails.join(', ')}
            </div>
            <div className="text-xs text-muted">
              <RequestTermsLine request={request} />
            </div>
            <div className="text-xs text-muted">
              {t('inviteRequest.askedBy', {
                name: request.requester_name,
                date: formatDay(request.expires_at),
              })}
            </div>
            {request.reason && (
              <p
                className="text-xs text-fg-soft whitespace-pre-wrap break-words"
                data-qa="invite-request-reason-text"
              >
                {t('inviteRequest.reasonQuoted', { reason: request.reason })}
              </p>
            )}
            <div className="flex flex-wrap gap-1.5 pt-1">
              <Button
                variant="primary"
                size="sm"
                disabled={approve.isPending}
                onClick={() => onApprove(request)}
                data-qa="invite-request-approve"
              >
                {t('inviteRequest.approve')}
              </Button>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setEditing(request)}
                data-qa="invite-request-edit"
              >
                {t('inviteRequest.edit')}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  setNote('');
                  setDeclining(request);
                }}
                data-qa="invite-request-decline"
              >
                {t('inviteRequest.decline')}
              </Button>
            </div>
          </div>
        </div>
      ))}
      {editing && (
        <InviteRequestDialog
          workspaceId={workspaceId}
          instanceUrl={instanceUrl}
          request={editing}
          onClose={() => setEditing(null)}
        />
      )}
      {declining && (
        <ConfirmDialog
          title={t('inviteRequest.declineTitle', { name: declining.requester_name })}
          description={t('inviteRequest.declineDescription')}
          confirmLabel={t('inviteRequest.decline')}
          onConfirm={async () => {
            await decline.mutateAsync({ requestId: declining.id, note });
            toast.success(t('inviteRequest.declined'));
          }}
          onClose={() => setDeclining(null)}
          dataQa="invite-request-decline-dialog"
        >
          <TextArea
            label={t('inviteRequest.note', { name: declining.requester_name })}
            optional
            rows={3}
            maxLength={MAX_NOTE}
            counter
            value={note}
            onChange={(e) => setNote(e.target.value)}
            dataQa="invite-request-note"
          />
        </ConfirmDialog>
      )}
    </div>
  );
}
