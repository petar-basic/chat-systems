import { useTranslation } from 'react-i18next';
import { Mail } from 'lucide-react';
import { formatDay } from '@/lib/datetime';
import { useInviteRequests } from '@/features/workspace/hooks/useInviteRequests';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
}

export function MyInviteRequests({ workspaceId, instanceUrl }: Props) {
  const { t } = useTranslation('workspace');
  const { data: requests = [] } = useInviteRequests(workspaceId, instanceUrl);
  if (requests.length === 0) return null;

  return (
    <div className="pb-2" data-qa="my-invite-requests">
      <div className="px-4 mb-1">
        <span className="text-xs font-semibold text-muted uppercase tracking-wider">
          {t('inviteRequest.mineTitle', { count: requests.length })}
        </span>
      </div>
      {requests.map((request) => (
        <div key={request.id} data-qa="my-invite-request-row" className="flex items-center gap-3 px-4 py-2">
          <Mail className="w-4 h-4 shrink-0 text-muted" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm text-fg-soft">{request.emails.join(', ')}</div>
            <div className="text-xs text-muted">
              {t(request.role === 'guest' ? 'inviteRequest.asGuest' : 'inviteRequest.asMember')} ·{' '}
              {t('inviteRequest.sentOn', { date: formatDay(request.created_at) })}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
