import { RadioCards } from '@/shared/components/Form/RadioCards';
import type { Density, Previews } from '@/stores/display';
import { usePreferences, useUpdatePreferences } from './hooks/usePreferences';

export default function MessagesMediaPanel() {
  const { data } = usePreferences();
  const update = useUpdatePreferences();

  return (
    <div className="space-y-6" data-qa="messages-media">
      <RadioCards<Density>
        label="Message layout"
        value={data?.density ?? 'clean'}
        onChange={(density) => update.mutate({ density })}
        options={[
          {
            value: 'clean',
            label: 'Clean',
            description: 'Name and photo above each run of messages.',
            dataQa: 'density-clean',
          },
          {
            value: 'compact',
            label: 'Compact',
            description: 'Name and time on the line of every message, with less space between them.',
            dataQa: 'density-compact',
          },
        ]}
      />
      <RadioCards<Previews>
        label="Images and link previews"
        value={data?.previews ?? 'shown'}
        onChange={(previews) => update.mutate({ previews })}
        options={[
          {
            value: 'shown',
            label: 'Show in the message',
            description: 'Pictures and previews of links open as the message arrives.',
            dataQa: 'previews-shown',
          },
          {
            value: 'collapsed',
            label: 'Collapse until clicked',
            description: 'A line in their place opens them when you want them.',
            dataQa: 'previews-collapsed',
          },
        ]}
      />
      <p className="text-sm text-muted">
        These settings are the same in every workspace and on every device.
      </p>
      {update.isError && (
        <p role="alert" className="text-sm text-danger">
          The setting could not be saved. Try again.
        </p>
      )}
    </div>
  );
}
