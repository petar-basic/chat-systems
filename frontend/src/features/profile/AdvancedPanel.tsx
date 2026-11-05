import { useTranslation } from 'react-i18next';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { isApplePlatform, shortcutLabel } from '@/features/navigation';
import type { EnterKey } from '@/stores/display';
import { usePreferences, useUpdatePreferences } from './hooks/usePreferences';
import SavedEverywhere from './SavedEverywhere';

export default function AdvancedPanel() {
  const { t } = useTranslation('profile');
  const { data } = usePreferences();
  const update = useUpdatePreferences();
  const apple = isApplePlatform();
  const send = shortcutLabel(['mod', 'Enter'], apple);
  const newLine = shortcutLabel(['shift', 'Enter'], apple);

  return (
    <div className="space-y-6" data-qa="advanced">
      <RadioCards<EnterKey>
        label={t('advanced.enter')}
        value={data?.enter_key ?? 'send'}
        onChange={(enter_key) => update.mutate({ enter_key })}
        options={[
          {
            value: 'send',
            label: t('advanced.sends.label'),
            description: t('advanced.sends.description', { keys: newLine }),
            dataQa: 'enter-sends',
          },
          {
            value: 'newline',
            label: t('advanced.newLine.label'),
            description: t('advanced.newLine.description', { keys: send }),
            dataQa: 'enter-newline',
          },
        ]}
      />
      <SavedEverywhere failed={update.isError} />
    </div>
  );
}
