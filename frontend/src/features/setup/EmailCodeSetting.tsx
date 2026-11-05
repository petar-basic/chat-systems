import { Checkbox } from '@/shared/components/Form/Checkbox';
import { useInstanceSettings, useUpdateInstanceSettings } from '@/features/instance';
import { toUserMessage } from '@/lib/errors';

interface Props {
  instanceUrl: string | null;
}

export function EmailCodeSetting({ instanceUrl }: Props) {
  const { data } = useInstanceSettings(instanceUrl);
  const update = useUpdateInstanceSettings(instanceUrl);
  if (!data) return null;

  return (
    <div className="space-y-2 border-t border-line pt-6" data-qa="email-code-setting">
      <Checkbox
        label="Sign in with an emailed code"
        description={
          data.email_configured
            ? 'The sign-in page offers to email a six-digit code, so nobody needs a password to get in.'
            : 'Needs SMTP. Until this instance can send email, the sign-in page does not offer it.'
        }
        checked={data.email_code_sign_in && data.email_configured}
        disabled={!data.email_configured || update.isPending}
        onChange={(on) => update.mutate({ email_code_sign_in: on })}
        dataQa="email-code-toggle"
      />
      {update.isError && (
        <p className="text-sm text-danger" role="alert">
          {toUserMessage(update.error, 'Could not save the setting')}
        </p>
      )}
    </div>
  );
}
