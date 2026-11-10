import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import type { ChannelRetention } from '@/features/workspace';
import { periodOf, type PeriodChoice } from './channelPeriod';

function workspacePeriod(days: number | null | undefined, t: TFunction<'channel'>) {
  return days ? t('details.retention.days', { count: days }) : t('details.retention.forever');
}

interface Props {
  retention: ChannelRetention;
  choice: PeriodChoice;
  onChange: (choice: PeriodChoice) => void;
}

export function ChannelRetentionField({ retention, choice, onChange }: Props) {
  const { t } = useTranslation('channel');
  const days = retention.workspace_message_days;
  const workspace = workspacePeriod(days, t);

  if (!retention.overrides_allowed) {
    return (
      <p className="text-sm text-muted" data-qa="channel-retention-workspace">
        {days ? t('details.retention.followsDays', { count: days }) : t('details.retention.followsForever')}
      </p>
    );
  }

  return (
    <div className="space-y-2" data-qa="channel-retention">
      <Select
        label={t('details.retention.label')}
        value={choice.mode}
        onChange={(e) => {
          const mode = e.target.value;
          if (mode === 'workspace' || mode === 'forever' || mode === 'days') onChange({ ...choice, mode });
        }}
        dataQa="channel-retention-mode"
      >
        <option value="workspace">{t('details.retention.workspaceDefault', { period: workspace })}</option>
        <option value="forever">{t('details.retention.keepForever')}</option>
        <option value="days">{t('details.retention.someDays')}</option>
      </Select>
      {choice.mode === 'days' && (
        <TextInput
          label={t('details.retention.daysLabel')}
          type="number"
          min={1}
          inputMode="numeric"
          value={choice.days}
          onChange={(e) => onChange({ ...choice, days: e.target.value })}
          error={periodOf(choice) === 'invalid' ? t('details.retention.atLeastOne') : undefined}
          dataQa="channel-retention-days"
          className="max-w-48"
        />
      )}
    </div>
  );
}
