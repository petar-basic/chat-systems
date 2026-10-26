import { Mail } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { useSendTestEmail } from '@/features/instance/hooks/useAdmin';
import { toUserMessage } from '@/lib/errors';

interface Props {
  instanceUrl: string | null;
}

export function TestEmailCheck({ instanceUrl }: Props) {
  const send = useSendTestEmail(instanceUrl);
  const result = send.data;

  return (
    <div className="space-y-3" data-qa="test-email">
      <p className="text-sm text-muted">
        Invites and password resets go out by email. Send yourself one to check that the SMTP settings work.
      </p>
      <Button
        icon={<Mail className="h-4 w-4" />}
        onClick={() => send.mutate()}
        loading={send.isPending}
        data-qa="test-email-send"
      >
        Send a test email
      </Button>
      <div aria-live="polite" className="text-sm">
        {send.isError ? (
          <p className="text-danger" data-qa="test-email-failed">
            {toUserMessage(send.error, 'Could not send the test email')}
          </p>
        ) : result?.error ? (
          <p className="text-danger" data-qa="test-email-failed">
            {result.error}
          </p>
        ) : result ? (
          <p className="text-success" data-qa="test-email-sent">
            Sent to {result.sent_to}. Check that inbox, and the spam folder.
          </p>
        ) : null}
      </div>
    </div>
  );
}
