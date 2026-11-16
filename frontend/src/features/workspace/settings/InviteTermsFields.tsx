import { useTranslation } from 'react-i18next';
import { Select } from '@/shared/components/Form/Select';
import { TextArea } from '@/shared/components/Form/TextArea';
import { GuestInviteFields } from './GuestInviteFields';
import type { GuestInvite } from './guestTerms';
import { MAX_EMAILS, parseEmails, type RequestRole } from './inviteRequestTerms';

export interface InviteTermsValue {
  emails: string;
  role: RequestRole;
  guest: GuestInvite;
}

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  value: InviteTermsValue;
  onChange: (value: InviteTermsValue) => void;
}

export function InviteTermsFields({ workspaceId, instanceUrl, value, onChange }: Props) {
  const { t } = useTranslation('workspace');
  const tooMany = parseEmails(value.emails).length > MAX_EMAILS;

  return (
    <div className="space-y-2">
      <TextArea
        label={t('inviteRequest.emails')}
        hint={t('inviteRequest.emailsHint', { max: MAX_EMAILS })}
        error={tooMany ? t('inviteRequest.tooMany', { max: MAX_EMAILS }) : undefined}
        rows={2}
        value={value.emails}
        onChange={(e) => onChange({ ...value, emails: e.target.value })}
        placeholder="user@example.com"
        autoFocus
        dataQa="invite-request-emails"
      />
      <Select
        label={t('inviteRequest.role')}
        value={value.role}
        onChange={(e) => onChange({ ...value, role: e.target.value === 'guest' ? 'guest' : 'member' })}
        dataQa="invite-request-role"
      >
        <option value="member">{t('inviteRequest.member')}</option>
        <option value="guest">{t('inviteRequest.guest')}</option>
      </Select>
      {value.role === 'guest' && (
        <GuestInviteFields
          workspaceId={workspaceId}
          instanceUrl={instanceUrl}
          value={value.guest}
          onChange={(guest) => onChange({ ...value, guest })}
        />
      )}
    </div>
  );
}
