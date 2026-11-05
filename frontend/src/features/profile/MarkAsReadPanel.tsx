import { useTranslation } from 'react-i18next';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import type { OpenChannelAt } from '@/stores/display';
import { usePreferences, useUpdatePreferences } from './hooks/usePreferences';
import SavedEverywhere from './SavedEverywhere';

export default function MarkAsReadPanel() {
  const { t } = useTranslation('profile');
  const { data } = usePreferences();
  const update = useUpdatePreferences();

  return (
    <div className="space-y-6" data-qa="mark-as-read">
      <RadioCards<OpenChannelAt>
        label={t('read.label')}
        value={data?.open_channel_at ?? 'where_left'}
        onChange={(open_channel_at) => update.mutate({ open_channel_at })}
        options={[
          {
            value: 'where_left',
            label: t('read.whereLeft.label'),
            description: t('read.whereLeft.description'),
            dataQa: 'open-channel-where-left',
          },
          {
            value: 'newest',
            label: t('read.newest.label'),
            description: t('read.newest.description'),
            dataQa: 'open-channel-newest',
          },
        ]}
      />
      <SavedEverywhere failed={update.isError} />
    </div>
  );
}
