import { Mail } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import { useSendTestEmail } from '@/features/instance';
import { toUserMessage } from '@/lib/errors';

interface Props {
  instanceUrl: string | null;
}

export function TestEmailCheck({ instanceUrl }: Props) {
  const { t } = useTranslation('admin');
  const send = useSendTestEmail(instanceUrl);
  const result = send.data;

  return (
    <div className="space-y-3" data-qa="test-email">
      <p className="text-sm text-muted">{t('testEmail.intro')}</p>
      <Button
        icon={<Mail className="h-4 w-4" />}
        onClick={() => send.mutate()}
        loading={send.isPending}
        data-qa="test-email-send"
      >
        {t('testEmail.send')}
      </Button>
      <div aria-live="polite" className="text-sm">
        {send.isError ? (
          <p className="text-danger" data-qa="test-email-failed">
            {toUserMessage(send.error, t('testEmail.failed'))}
          </p>
        ) : result?.error ? (
          <p className="text-danger" data-qa="test-email-failed">
            {result.error}
          </p>
        ) : result ? (
          <p className="text-success" data-qa="test-email-sent">
            {t('testEmail.sent', { email: result.sent_to })}
          </p>
        ) : null}
      </div>
    </div>
  );
}
