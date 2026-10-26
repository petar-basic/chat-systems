import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import type { ChannelRetention } from '@/features/workspace';
import { periodOf, type PeriodChoice } from './channelPeriod';

function workspacePeriod(days: number | null | undefined) {
  return days ? `${days} day${days === 1 ? '' : 's'}` : 'forever';
}

interface Props {
  retention: ChannelRetention;
  choice: PeriodChoice;
  onChange: (choice: PeriodChoice) => void;
}

export function ChannelRetentionField({ retention, choice, onChange }: Props) {
  const workspace = workspacePeriod(retention.workspace_message_days);

  if (!retention.overrides_allowed) {
    return (
      <p className="text-sm text-muted" data-qa="channel-retention-workspace">
        Messages here follow the workspace and are kept{' '}
        {workspace === 'forever' ? 'forever' : `for ${workspace}`}.
      </p>
    );
  }

  return (
    <div className="space-y-2" data-qa="channel-retention">
      <Select
        label="Message retention"
        value={choice.mode}
        onChange={(e) => {
          const mode = e.target.value;
          if (mode === 'workspace' || mode === 'forever' || mode === 'days') onChange({ ...choice, mode });
        }}
        dataQa="channel-retention-mode"
      >
        <option value="workspace">Workspace default ({workspace})</option>
        <option value="forever">Keep forever</option>
        <option value="days">A number of days</option>
      </Select>
      {choice.mode === 'days' && (
        <TextInput
          label="Days to keep messages"
          type="number"
          min={1}
          inputMode="numeric"
          value={choice.days}
          onChange={(e) => onChange({ ...choice, days: e.target.value })}
          error={periodOf(choice) === 'invalid' ? 'At least one day' : undefined}
          dataQa="channel-retention-days"
          className="max-w-48"
        />
      )}
    </div>
  );
}
