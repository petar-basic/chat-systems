import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue';
import { toUserMessage } from '@/lib/errors';
import {
  useRetentionPolicy,
  useRetentionPreview,
  useSaveRetention,
  type RetentionPolicy,
} from '@/features/workspace/hooks/useRetention';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
}

type Keep = 'forever' | 'days';

interface Period {
  keep: Keep;
  days: string;
}

function periodOf(days: number | null | undefined): Period {
  return days ? { keep: 'days', days: String(days) } : { keep: 'forever', days: '365' };
}

function daysOf(period: Period): number | null | 'invalid' {
  if (period.keep === 'forever') return null;
  const days = Number(period.days);
  return Number.isInteger(days) && days >= 1 ? days : 'invalid';
}

function PeriodField({
  label,
  daysLabel,
  period,
  onChange,
  qa,
}: {
  label: string;
  daysLabel: string;
  period: Period;
  onChange: (period: Period) => void;
  qa: string;
}) {
  const { t } = useTranslation('workspace');
  return (
    <div className="space-y-2" data-qa={qa}>
      <RadioCards
        label={label}
        value={period.keep}
        onChange={(keep) => onChange({ ...period, keep })}
        options={[
          { value: 'forever', label: t('settings.retention.forever'), dataQa: `${qa}-forever` },
          { value: 'days', label: t('settings.retention.forDays'), dataQa: `${qa}-days` },
        ]}
      />
      {period.keep === 'days' && (
        <TextInput
          label={daysLabel}
          type="number"
          min={1}
          inputMode="numeric"
          value={period.days}
          onChange={(e) => onChange({ ...period, days: e.target.value })}
          error={daysOf(period) === 'invalid' ? t('settings.retention.atLeastOneDay') : undefined}
          dataQa={`${qa}-input`}
          className="max-w-48"
        />
      )}
    </div>
  );
}

function RetentionForm({
  workspaceId,
  instanceUrl,
  policy,
}: Props & {
  policy: RetentionPolicy | null;
}) {
  const { t } = useTranslation('workspace');
  const [messages, setMessages] = useState(periodOf(policy?.message_days));
  const [files, setFiles] = useState(periodOf(policy?.file_days));
  const [channelOverrides, setChannelOverrides] = useState(policy?.channel_overrides ?? false);
  const save = useSaveRetention(workspaceId, instanceUrl);

  const messageDays = daysOf(messages);
  const fileDays = daysOf(files);
  const valid = messageDays !== 'invalid' && fileDays !== 'invalid';
  const changed =
    valid &&
    (messageDays !== (policy?.message_days ?? null) ||
      fileDays !== (policy?.file_days ?? null) ||
      channelOverrides !== (policy?.channel_overrides ?? false));

  const settledMessages = useDebouncedValue(valid ? messageDays : null, 300);
  const settledFiles = useDebouncedValue(valid ? fileDays : null, 300);
  const preview = useRetentionPreview(
    workspaceId,
    { messageDays: settledMessages, fileDays: settledFiles, channelOverrides },
    instanceUrl,
  );
  const removesNothing = settledMessages === null && settledFiles === null && !channelOverrides;

  const submit = () => {
    if (!valid) return;
    save.mutate({
      message_days: messageDays,
      file_days: fileDays,
      audit_days: policy?.audit_days,
      notification_days: policy?.notification_days,
      channel_overrides: channelOverrides,
    });
  };

  return (
    <div className="space-y-5" data-qa="retention-section">
      <p className="text-sm text-muted">{t('settings.retention.intro')}</p>

      <PeriodField
        label={t('settings.retention.keepMessages')}
        daysLabel={t('settings.retention.daysMessages')}
        period={messages}
        onChange={setMessages}
        qa="retention-messages"
      />
      <PeriodField
        label={t('settings.retention.keepFiles')}
        daysLabel={t('settings.retention.daysFiles')}
        period={files}
        onChange={setFiles}
        qa="retention-files"
      />

      <Checkbox
        label={t('settings.retention.channelOverrides')}
        description={t('settings.retention.channelOverridesHint')}
        checked={channelOverrides}
        onChange={setChannelOverrides}
        dataQa="retention-channel-overrides"
      />

      <p aria-live="polite" className="min-h-5 text-sm" data-qa="retention-preview">
        {removesNothing ? (
          <span className="text-muted">{t('settings.retention.nothingRemoved')}</span>
        ) : preview.isPending ? (
          <span className="text-muted">{t('settings.retention.counting')}</span>
        ) : preview.isError ? (
          <span className="text-danger">
            {toUserMessage(preview.error, t('settings.retention.countFailed'))}
          </span>
        ) : (
          <span className="text-warning">
            {t('settings.retention.removedNow', {
              messages: t('settings.retention.messages', {
                count: preview.data.messages,
                n: preview.data.messages.toLocaleString(),
              }),
              files: t('settings.retention.files', {
                count: preview.data.files,
                n: preview.data.files.toLocaleString(),
              }),
            })}
          </span>
        )}
      </p>

      <div className="flex items-center gap-3">
        <Button
          variant="primary"
          onClick={submit}
          disabled={!changed}
          loading={save.isPending}
          data-qa="retention-save"
        >
          {t('settings.retention.save')}
        </Button>
        <span aria-live="polite" className="text-xs" data-qa="retention-save-status">
          {save.isError ? (
            <span className="text-danger">
              {toUserMessage(save.error, t('settings.retention.saveFailed'))}
            </span>
          ) : save.isSuccess && !changed ? (
            <span className="text-success">{t('settings.retention.saved')}</span>
          ) : null}
        </span>
      </div>
    </div>
  );
}

export function RetentionSection({ workspaceId, instanceUrl }: Props) {
  const { t } = useTranslation('workspace');
  const { data: policy, isLoading, error } = useRetentionPolicy(workspaceId, instanceUrl);

  if (isLoading) {
    return (
      <div className="flex justify-center py-8">
        <Spinner className="text-accent" />
      </div>
    );
  }
  if (error)
    return <p className="text-sm text-danger">{toUserMessage(error, t('settings.retention.loadFailed'))}</p>;
  return <RetentionForm workspaceId={workspaceId} instanceUrl={instanceUrl} policy={policy ?? null} />;
}
