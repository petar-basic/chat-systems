import { Checkbox } from '@/shared/components/Form/Checkbox';
import { useTranslation } from 'react-i18next';
import { useInstanceSettings, useUpdateInstanceSettings } from '@/features/instance';
import { toUserMessage } from '@/lib/errors';

interface Props {
  instanceUrl: string | null;
}

export function OtherServersSetting({ instanceUrl }: Props) {
  const { t } = useTranslation('admin');
  const { data } = useInstanceSettings(instanceUrl);
  const update = useUpdateInstanceSettings(instanceUrl);
  if (!data) return null;

  return (
    <div className="space-y-2 border-t border-line pt-6" data-qa="other-servers-setting">
      <Checkbox
        label={t('otherServers.label')}
        description={data.other_servers_allowed ? t('otherServers.on') : t('otherServers.off')}
        checked={data.other_servers_allowed}
        disabled={update.isPending}
        onChange={(on) => update.mutate({ other_servers_allowed: on })}
        dataQa="other-servers-toggle"
      />
      {update.isError && (
        <p className="text-sm text-danger" role="alert">
          {toUserMessage(update.error, t('saveFailed'))}
        </p>
      )}
    </div>
  );
}
