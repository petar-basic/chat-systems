import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { Combobox, type ComboboxOption } from '@/shared/components/Form/Combobox';
import { useWorkspaceStore } from '@/stores/workspace';
import type { Clock } from '@/stores/display';
import {
  TIME,
  browserTimezone,
  formatClock,
  localTime,
  matchesTimezone,
  timezoneChoices,
} from '@/lib/datetime';
import { usePreferences, useUpdatePreferences } from './hooks/usePreferences';
import { useAccount } from './hooks/useProfile';
import { useUpdateTimezone } from './hooks/useTimezone';
import LanguagePicker from './LanguagePicker';
import { rememberAsked } from './timezoneQuestion';
import SavedEverywhere from './SavedEverywhere';

const EXAMPLE = new Date(2026, 0, 1, 14, 5);

export default function RegionPanel() {
  const { t } = useTranslation('profile');
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const { data: account } = useAccount(instanceUrl);
  const { data: preferences } = usePreferences();
  const updatePreferences = useUpdatePreferences();
  const updateTimezone = useUpdateTimezone();

  const timezone = account?.timezone ?? browserTimezone();
  const clock = preferences?.clock ?? 'h24';
  const now = localTime(timezone, new Date(), clock);
  const hint = now ? t('region.now', { time: now }) : undefined;

  const choices = useMemo(() => new Map(timezoneChoices(timezone).map((c) => [c.zone, c])), [timezone]);
  const options = useMemo(() => {
    const at = new Date();
    return [...choices.values()].map((c) => ({
      value: c.zone,
      label: c.city,
      detail: [c.region, c.offset, localTime(c.zone, at, clock)].filter(Boolean).join(' · '),
    }));
  }, [choices, clock]);
  const filter = useCallback(
    (option: ComboboxOption, query: string) => {
      const choice = choices.get(option.value);
      return choice ? matchesTimezone(choice, query) : false;
    },
    [choices],
  );

  return (
    <div className="space-y-6" data-qa="region">
      <LanguagePicker />
      <div className="space-y-2">
        <Combobox
          label={t('region.timezone')}
          hint={hint}
          value={timezone}
          options={options}
          filter={filter}
          onChange={(zone) => {
            rememberAsked(browserTimezone());
            updateTimezone.mutate(zone);
          }}
          placeholder={t('region.find')}
          emptyText={t('region.noMatch')}
          disabled={!account}
          dataQa="timezone-picker"
        />
        {updateTimezone.isError && (
          <p role="alert" className="text-sm text-danger">
            {t('region.saveFailed')}
          </p>
        )}
      </div>
      <RadioCards<Clock>
        label={t('region.clock')}
        value={clock}
        onChange={(next) => updatePreferences.mutate({ clock: next })}
        options={[
          {
            value: 'h24',
            label: t('region.h24'),
            description: formatClock(EXAMPLE, TIME, 'h24'),
            dataQa: 'clock-h24',
          },
          {
            value: 'h12',
            label: t('region.h12'),
            description: formatClock(EXAMPLE, TIME, 'h12'),
            dataQa: 'clock-h12',
          },
        ]}
      />
      <SavedEverywhere failed={updatePreferences.isError} />
    </div>
  );
}
