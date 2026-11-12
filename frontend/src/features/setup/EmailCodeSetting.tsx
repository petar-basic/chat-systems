import { Checkbox } from '@/shared/components/Form/Checkbox';
import { useTranslation } from 'react-i18next';
import { useInstanceSettings, useUpdateInstanceSettings } from '@/features/instance';
import { toUserMessage } from '@/lib/errors';

interface Props {
  instanceUrl: string | null;
}

export function EmailCodeSetting({ instanceUrl }: Props) {
  const { t } = useTranslation('admin');
  const { data } = useInstanceSettings(instanceUrl);
  const update = useUpdateInstanceSettings(instanceUrl);
  if (!data) return null;

  return (
    <div className="space-y-2 border-t border-line pt-6" data-qa="email-code-setting">
      <Checkbox
        label={t('emailCode.label')}
        description={data.email_configured ? t('emailCode.on') : t('emailCode.off')}
        checked={data.email_code_sign_in && data.email_configured}
        disabled={!data.email_configured || update.isPending}
        onChange={(on) => update.mutate({ email_code_sign_in: on })}
        dataQa="email-code-toggle"
      />
      {update.isError && (
        <p className="text-sm text-danger" role="alert">
          {toUserMessage(update.error, t('saveFailed'))}
        </p>
      )}
    </div>
  );
}
