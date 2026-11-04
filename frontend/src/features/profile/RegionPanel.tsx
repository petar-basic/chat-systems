import { useState } from 'react';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import { useWorkspaceStore } from '@/stores/workspace';
import type { Clock } from '@/stores/display';
import { TIME, browserTimezone, formatClock, localTime, timezoneChoices } from '@/lib/datetime';
import { usePreferences, useUpdatePreferences } from './hooks/usePreferences';
import { useAccount } from './hooks/useProfile';
import { useUpdateTimezone } from './hooks/useTimezone';
import LanguagePicker from './LanguagePicker';
import SavedEverywhere from './SavedEverywhere';

const EXAMPLE = new Date(2026, 0, 1, 14, 5);

export default function RegionPanel() {
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  const { data: account } = useAccount(instanceUrl);
  const { data: preferences } = usePreferences();
  const updatePreferences = useUpdatePreferences();
  const updateTimezone = useUpdateTimezone();
  const [search, setSearch] = useState('');

  const timezone = account?.timezone ?? browserTimezone();
  const clock = preferences?.clock ?? 'h24';
  const now = localTime(timezone, new Date(), clock);

  return (
    <div className="space-y-6" data-qa="region">
      <LanguagePicker />
      <div className="space-y-2">
        <Select
          label="Time zone"
          hint={now ? `It is ${now} for you. People see this on your profile.` : undefined}
          value={timezone}
          onChange={(e) => updateTimezone.mutate(e.target.value)}
          disabled={!account}
          dataQa="timezone-picker"
        >
          {timezoneChoices(timezone, search).map((zone) => (
            <option key={zone} value={zone}>
              {zone.replace(/_/g, ' ')}
            </option>
          ))}
        </Select>
        <TextInput
          label="Find a time zone"
          labelHidden
          placeholder="Find a time zone, e.g. Tokyo"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          dataQa="timezone-search"
        />
        {updateTimezone.isError && (
          <p role="alert" className="text-sm text-danger">
            The time zone could not be saved. Try again.
          </p>
        )}
      </div>
      <RadioCards<Clock>
        label="Clock"
        value={clock}
        onChange={(next) => updatePreferences.mutate({ clock: next })}
        options={[
          {
            value: 'h24',
            label: '24-hour',
            description: formatClock(EXAMPLE, TIME, 'h24'),
            dataQa: 'clock-h24',
          },
          {
            value: 'h12',
            label: '12-hour',
            description: formatClock(EXAMPLE, TIME, 'h12'),
            dataQa: 'clock-h12',
          },
        ]}
      />
      <SavedEverywhere failed={updatePreferences.isError} />
    </div>
  );
}
