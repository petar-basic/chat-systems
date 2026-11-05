import { useTranslation } from 'react-i18next';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import type { Density, Previews } from '@/stores/display';
import { usePreferences, useUpdatePreferences } from './hooks/usePreferences';
import SavedEverywhere from './SavedEverywhere';

export default function MessagesMediaPanel() {
  const { t } = useTranslation('profile');
  const { data } = usePreferences();
  const update = useUpdatePreferences();

  return (
    <div className="space-y-6" data-qa="messages-media">
      <RadioCards<Density>
        label={t('messages.layout')}
        value={data?.density ?? 'clean'}
        onChange={(density) => update.mutate({ density })}
        options={[
          {
            value: 'clean',
            label: t('messages.clean.label'),
            description: t('messages.clean.description'),
            dataQa: 'density-clean',
          },
          {
            value: 'compact',
            label: t('messages.compact.label'),
            description: t('messages.compact.description'),
            dataQa: 'density-compact',
          },
        ]}
      />
      <RadioCards<Previews>
        label={t('messages.previews')}
        value={data?.previews ?? 'shown'}
        onChange={(previews) => update.mutate({ previews })}
        options={[
          {
            value: 'shown',
            label: t('messages.shown.label'),
            description: t('messages.shown.description'),
            dataQa: 'previews-shown',
          },
          {
            value: 'collapsed',
            label: t('messages.collapsed.label'),
            description: t('messages.collapsed.description'),
            dataQa: 'previews-collapsed',
          },
        ]}
      />
      <SavedEverywhere failed={update.isError} />
    </div>
  );
}
