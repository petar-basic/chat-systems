import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { toUserMessage } from '@/lib/errors';
import { type InviteRequest, useApproveInviteRequest } from '@/features/workspace/hooks/useInviteRequests';
import { toast } from '@/shared/components/Toast';
import { guestOf, parseEmails, readyToSend, roleOf, termsFor } from './inviteRequestTerms';
import { InviteTermsFields, type InviteTermsValue } from './InviteTermsFields';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  request: InviteRequest;
  onClose: () => void;
}

export function InviteRequestDialog({ workspaceId, instanceUrl, request, onClose }: Props) {
  const { t } = useTranslation(['workspace', 'common']);
  const approve = useApproveInviteRequest(workspaceId, instanceUrl);
  const [terms, setTerms] = useState<InviteTermsValue>({
    emails: request.emails.join('\n'),
    role: roleOf(request),
    guest: guestOf(request),
  });
  const [error, setError] = useState<string | null>(null);
  const emails = parseEmails(terms.emails);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      const approved = await approve.mutateAsync({
        requestId: request.id,
        terms: termsFor(emails, terms.role, terms.guest),
      });
      toast.success(t('inviteRequest.approved', { count: approved.invites.length }));
      onClose();
    } catch (err) {
      setError(toUserMessage(err, t('inviteRequest.approveFailed')));
    }
  };

  return (
    <Modal
      title={t('inviteRequest.editTitle', { name: request.requester_name })}
      description={t('inviteRequest.editDescription')}
      onClose={onClose}
      onSubmit={handleSubmit}
      dataQa="invite-request-dialog"
      dismissLabel={t('cancel', { ns: 'common' })}
      actions={
        <Button
          type="submit"
          variant="primary"
          loading={approve.isPending}
          disabled={!readyToSend(emails, terms.role, terms.guest)}
          data-qa="invite-request-dialog-approve"
        >
          {t('inviteRequest.approveAndInvite')}
        </Button>
      }
    >
      <InviteTermsFields
        workspaceId={workspaceId}
        instanceUrl={instanceUrl}
        value={terms}
        onChange={setTerms}
      />
      {error && (
        <p className="mt-2 text-xs text-danger" data-qa="invite-request-dialog-error">
          {error}
        </p>
      )}
    </Modal>
  );
}
