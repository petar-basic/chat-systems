import { RadioCards } from '@/shared/components/Form/RadioCards';
import { useTranslation } from 'react-i18next';
import { useInstanceSettings, useUpdateInstanceSettings } from '@/features/instance';
import { toUserMessage } from '@/lib/errors';

interface Props {
  instanceUrl: string | null;
}

type Who = 'everyone' | 'creators';

export function WorkspaceCreationSetting({ instanceUrl }: Props) {
  const { t } = useTranslation('admin');
  const { data } = useInstanceSettings(instanceUrl);
  const update = useUpdateInstanceSettings(instanceUrl);
  if (!data) return null;

  const who: Who = data.workspace_creation_restricted ? 'creators' : 'everyone';

  return (
    <div className="space-y-2" data-qa="workspace-creation-setting">
      <RadioCards<Who>
        label={t('creation.label')}
        value={who}
        onChange={(next) => {
          if (next !== who && !update.isPending)
            update.mutate({ workspace_creation_restricted: next === 'creators' });
        }}
        options={[
          {
            value: 'creators',
            label: t('creation.creators'),
            description: t('creation.creatorsHint'),
            dataQa: 'workspace-creation-creators',
          },
          {
            value: 'everyone',
            label: t('creation.everyone'),
            description: t('creation.everyoneHint'),
            dataQa: 'workspace-creation-everyone',
          },
        ]}
      />
      {update.isError && (
        <p className="text-sm text-danger" role="alert">
          {toUserMessage(update.error, t('saveFailed'))}
        </p>
      )}
    </div>
  );
}
