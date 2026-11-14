import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import { TextArea } from '@/shared/components/Form/TextArea';
import { toUserMessage } from '@/lib/errors';
import { useAskForInvites } from '@/features/workspace/hooks/useInviteRequests';
import { NO_GUEST } from './guestTerms';
import { MAX_NOTE, parseEmails, readyToSend, termsFor } from './inviteRequestTerms';
import { InviteTermsFields, type InviteTermsValue } from './InviteTermsFields';

const BLANK: InviteTermsValue = { emails: '', role: 'member', guest: NO_GUEST };

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  onClose: () => void;
}

export function InviteRequestForm({ workspaceId, instanceUrl, onClose }: Props) {
  const { t } = useTranslation(['workspace', 'common']);
  const ask = useAskForInvites(workspaceId, instanceUrl);
  const [terms, setTerms] = useState<InviteTermsValue>(BLANK);
  const [reason, setReason] = useState('');
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const emails = parseEmails(terms.emails);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setSent(false);
    try {
      await ask.mutateAsync({ ...termsFor(emails, terms.role, terms.guest), reason: reason.trim() || null });
      setTerms(BLANK);
      setReason('');
      setSent(true);
    } catch (err) {
      setError(toUserMessage(err, t('inviteRequest.sendFailed')));
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-2" data-qa="invite-request-form">
      <p className="text-xs text-muted px-1">{t('inviteRequest.explain')}</p>
      <InviteTermsFields
        workspaceId={workspaceId}
        instanceUrl={instanceUrl}
        value={terms}
        onChange={setTerms}
      />
      <TextArea
        label={t('inviteRequest.reason')}
        optional
        rows={2}
        maxLength={MAX_NOTE}
        counter
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        dataQa="invite-request-reason"
      />
      {error && (
        <p className="text-xs text-danger px-1" data-qa="invite-request-error">
          {error}
        </p>
      )}
      {sent && (
        <p className="text-xs text-success px-1" data-qa="invite-request-sent">
          {t('inviteRequest.sent')}
        </p>
      )}
      <div className="flex gap-2">
        <Button variant="ghost" className="flex-1" onClick={onClose}>
          {t('cancel', { ns: 'common' })}
        </Button>
        <Button
          variant="primary"
          className="flex-1"
          type="submit"
          loading={ask.isPending}
          disabled={!readyToSend(emails, terms.role, terms.guest)}
          data-qa="invite-request-submit"
        >
          {t('inviteRequest.send')}
        </Button>
      </div>
    </form>
  );
}
