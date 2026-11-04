import { RadioCards } from '@/shared/components/Form/RadioCards';
import type { OpenChannelAt } from '@/stores/display';
import { usePreferences, useUpdatePreferences } from './hooks/usePreferences';
import SavedEverywhere from './SavedEverywhere';

export default function MarkAsReadPanel() {
  const { data } = usePreferences();
  const update = useUpdatePreferences();

  return (
    <div className="space-y-6" data-qa="mark-as-read">
      <RadioCards<OpenChannelAt>
        label="When I open a channel"
        value={data?.open_channel_at ?? 'where_left'}
        onChange={(open_channel_at) => update.mutate({ open_channel_at })}
        options={[
          {
            value: 'where_left',
            label: 'Start where I left off',
            description: 'The first message you have not read, under a line that marks it.',
            dataQa: 'open-channel-where-left',
          },
          {
            value: 'newest',
            label: 'Start at the newest message',
            description: 'The latest message, with everything before it marked read.',
            dataQa: 'open-channel-newest',
          },
        ]}
      />
      <SavedEverywhere failed={update.isError} />
    </div>
  );
}
