import { RadioCards } from '@/shared/components/Form/RadioCards';
import { isApplePlatform, shortcutLabel } from '@/features/navigation';
import type { EnterKey } from '@/stores/display';
import { usePreferences, useUpdatePreferences } from './hooks/usePreferences';
import SavedEverywhere from './SavedEverywhere';

export default function AdvancedPanel() {
  const { data } = usePreferences();
  const update = useUpdatePreferences();
  const apple = isApplePlatform();
  const send = shortcutLabel(['mod', 'Enter'], apple);
  const newLine = shortcutLabel(['shift', 'Enter'], apple);

  return (
    <div className="space-y-6" data-qa="advanced">
      <RadioCards<EnterKey>
        label="When writing a message, Enter"
        value={data?.enter_key ?? 'send'}
        onChange={(enter_key) => update.mutate({ enter_key })}
        options={[
          {
            value: 'send',
            label: 'Sends it',
            description: `${newLine} starts a new line.`,
            dataQa: 'enter-sends',
          },
          {
            value: 'newline',
            label: 'Starts a new line',
            description: `${send} sends it.`,
            dataQa: 'enter-newline',
          },
        ]}
      />
      <SavedEverywhere failed={update.isError} />
    </div>
  );
}
