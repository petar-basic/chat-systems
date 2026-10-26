import { useState } from 'react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { TextInput } from '@/shared/components/Form/TextInput';
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue';
import { toUserMessage } from '@/lib/errors';
import {
  useRetentionPolicy,
  useRetentionPreview,
  useSaveRetention,
  type RetentionPolicy,
} from '@/hooks/queries/useRetention';

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

function plural(n: number, word: string) {
  return `${n.toLocaleString()} ${word}${n === 1 ? '' : 's'}`;
}

function PeriodField({
  label,
  noun,
  period,
  onChange,
  qa,
}: {
  label: string;
  noun: string;
  period: Period;
  onChange: (period: Period) => void;
  qa: string;
}) {
  return (
    <div className="space-y-2" data-qa={qa}>
      <RadioCards
        label={label}
        value={period.keep}
        onChange={(keep) => onChange({ ...period, keep })}
        options={[
          { value: 'forever', label: 'Forever', dataQa: `${qa}-forever` },
          { value: 'days', label: 'For a number of days', dataQa: `${qa}-days` },
        ]}
      />
      {period.keep === 'days' && (
        <TextInput
          label={`Days to keep ${noun}`}
          type="number"
          min={1}
          inputMode="numeric"
          value={period.days}
          onChange={(e) => onChange({ ...period, days: e.target.value })}
          error={daysOf(period) === 'invalid' ? 'At least one day' : undefined}
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
  const [messages, setMessages] = useState(periodOf(policy?.message_days));
  const [files, setFiles] = useState(periodOf(policy?.file_days));
  const save = useSaveRetention(workspaceId, instanceUrl);

  const messageDays = daysOf(messages);
  const fileDays = daysOf(files);
  const valid = messageDays !== 'invalid' && fileDays !== 'invalid';
  const changed =
    valid && (messageDays !== (policy?.message_days ?? null) || fileDays !== (policy?.file_days ?? null));

  const settledMessages = useDebouncedValue(valid ? messageDays : null, 300);
  const settledFiles = useDebouncedValue(valid ? fileDays : null, 300);
  const preview = useRetentionPreview(workspaceId, settledMessages, settledFiles, instanceUrl);

  const submit = () => {
    if (!valid) return;
    save.mutate({
      message_days: messageDays,
      file_days: fileDays,
      audit_days: policy?.audit_days,
      notification_days: policy?.notification_days,
    });
  };

  return (
    <div className="space-y-5" data-qa="retention-section">
      <p className="text-sm text-muted">
        Messages and files older than the period are deleted for good by a nightly job. There is no undo, and
        an export taken before the change is the only copy that remains.
      </p>

      <PeriodField
        label="Keep messages"
        noun="messages"
        period={messages}
        onChange={setMessages}
        qa="retention-messages"
      />
      <PeriodField label="Keep files" noun="files" period={files} onChange={setFiles} qa="retention-files" />

      <p aria-live="polite" className="min-h-5 text-sm" data-qa="retention-preview">
        {settledMessages === null && settledFiles === null ? (
          <span className="text-muted">Nothing is removed while both are kept forever.</span>
        ) : preview.isPending ? (
          <span className="text-muted">Counting…</span>
        ) : preview.isError ? (
          <span className="text-danger">{toUserMessage(preview.error, 'Could not count what would go')}</span>
        ) : (
          <span className="text-warning">
            About {plural(preview.data.messages, 'message')} and {plural(preview.data.files, 'file')} would be
            removed now.
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
          Save retention
        </Button>
        <span aria-live="polite" className="text-xs" data-qa="retention-save-status">
          {save.isError ? (
            <span className="text-danger">{toUserMessage(save.error, 'Failed to save retention')}</span>
          ) : save.isSuccess && !changed ? (
            <span className="text-success">Saved</span>
          ) : null}
        </span>
      </div>
    </div>
  );
}

export function RetentionSection({ workspaceId, instanceUrl }: Props) {
  const { data: policy, isLoading, error } = useRetentionPolicy(workspaceId, instanceUrl);

  if (isLoading) {
    return (
      <div className="flex justify-center py-8">
        <Spinner className="text-accent" />
      </div>
    );
  }
  if (error) return <p className="text-sm text-danger">{toUserMessage(error, 'Failed to load retention')}</p>;
  return <RetentionForm workspaceId={workspaceId} instanceUrl={instanceUrl} policy={policy ?? null} />;
}
