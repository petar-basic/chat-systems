import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Trans } from '@/shared/components/Trans/Trans';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { toUserMessage } from '@/lib/errors';
import { CheckCircle2, AlertTriangle, Loader2 } from 'lucide-react';
import {
  useSlackImports,
  useStartSlackImport,
  useRetrySlackImport,
  useDiscardSlackImport,
  workspaceNameFrom,
  type SlackImportRun,
  type SlackImportReport,
  reportOf,
} from '@/features/workspace/hooks/useSlackImport';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { TextInput } from '@/shared/components/Form/TextInput';
import { canCreateOn, useWorkspaceCreation } from '../hooks/useWorkspaceCreation';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { formatDateTime } from '@/lib/datetime';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
}

const COUNTS = [
  'messages_imported',
  'channels_created',
  'conversations_created',
  'users_created',
  'users_matched',
  'guests',
  'users_suspended',
  'channels_archived',
  'threads_resolved',
  'reactions',
  'files_imported',
  'emoji_imported',
] as const satisfies readonly (keyof Omit<SlackImportReport, 'skipped' | 'notes'>)[];

interface RunRowProps {
  run: SlackImportRun;
  retrying: boolean;
  onRetry: () => void;
  onDiscard: () => void;
}

function RunRow({ run, retrying, onRetry, onDiscard }: RunRowProps) {
  const { t } = useTranslation('workspace');
  const report = reportOf(run);
  const skipped = report.skipped ?? [];
  const notes = report.notes ?? [];
  const running = run.status === 'pending' || run.status === 'running';

  return (
    <div className="px-4 py-3 border-b border-line/40" data-qa="slack-import-run">
      <div className="flex items-center gap-2">
        {running && <Loader2 className="w-3.5 h-3.5 text-accent-soft animate-spin shrink-0" />}
        {run.status === 'complete' && <CheckCircle2 className="w-3.5 h-3.5 text-success shrink-0" />}
        {run.status === 'failed' && <AlertTriangle className="w-3.5 h-3.5 text-danger shrink-0" />}
        <span className="text-sm text-fg-soft truncate" data-qa="slack-import-source">
          {run.source}
        </span>
        {run.dry_run && (
          <span className="px-1.5 py-0.5 rounded bg-raised text-3xs uppercase tracking-wide text-fg-dim">
            {t('settings.slackImport.dryRunTag')}
          </span>
        )}
        <span className="ml-auto text-xs text-muted" data-qa="slack-import-status">
          {t(`settings.jobStatus.${run.status}`, { defaultValue: run.status })}
        </span>
      </div>

      <div className="mt-1.5 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-muted">
        {COUNTS.filter((key) => report[key] > 0).map((key) => (
          <span key={key}>
            <Trans
              t={t}
              i18nKey={`settings.slackImport.counts.${key}`}
              values={{ n: report[key] }}
              components={{ n: <span className="text-fg-soft tabular-nums" /> }}
            />
          </span>
        ))}
      </div>

      {notes.length > 0 && (
        <ul className="mt-1.5 space-y-0.5" data-qa="slack-import-notes">
          {notes.map((note, i) => (
            <li key={i} className="text-2xs text-muted">
              {note}
            </li>
          ))}
        </ul>
      )}

      {run.error && (
        <p className="mt-1.5 text-xs text-danger" data-qa="slack-import-error">
          {run.error}
        </p>
      )}

      {run.retryable && (
        <div className="mt-1.5 flex gap-2">
          <Button
            size="sm"
            onClick={onRetry}
            loading={retrying}
            disabled={retrying}
            data-qa="slack-import-retry"
          >
            {t('settings.slackImport.startAgain')}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={onDiscard}
            disabled={retrying}
            data-qa="slack-import-discard"
          >
            {t('settings.slackImport.discard')}
          </Button>
        </div>
      )}

      {run.status === 'failed' && run.archive_deleted_at && (
        <p className="mt-1.5 text-2xs text-muted" data-qa="slack-import-archive-gone">
          {t('settings.slackImport.archiveGone', { date: formatDateTime(run.archive_deleted_at) })}
        </p>
      )}

      {skipped.length > 0 && (
        <details className="mt-1.5">
          <summary className="text-xs text-muted cursor-pointer hover:text-fg-soft">
            {t('settings.slackImport.notImported', { count: skipped.length })}
          </summary>
          <ul className="mt-1 space-y-0.5" data-qa="slack-import-skipped">
            {skipped.slice(0, 50).map((item, i) => (
              <li key={i} className="text-2xs text-muted">
                <span className="text-fg-dim">{item.what}</span> — {item.why}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}

export function SlackImportSection({ workspaceId, instanceUrl }: Props) {
  const { t } = useTranslation('workspace');
  const { data: runs = [], isLoading } = useSlackImports(workspaceId, instanceUrl);
  const start = useStartSlackImport(workspaceId, instanceUrl);
  const retry = useRetrySlackImport(workspaceId, instanceUrl);
  const discard = useDiscardSlackImport(workspaceId, instanceUrl);
  const [discarding, setDiscarding] = useState<SlackImportRun | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const mayCreate = canCreateOn(useWorkspaceCreation(), instanceUrl);

  const [archive, setArchive] = useState<File | null>(null);
  const [dryRun, setDryRun] = useState(true);
  const [intoNew, setIntoNew] = useState(false);
  const [newName, setNewName] = useState('');
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    if (!archive) {
      setError(t('settings.slackImport.chooseZip'));
      return;
    }
    if (intoNew && mayCreate && !newName.trim()) {
      setError(t('settings.slackImport.nameRequired'));
      return;
    }
    setError(null);
    try {
      await start.mutateAsync({
        archive,
        dryRun,
        newWorkspaceName: intoNew && mayCreate ? newName.trim() : undefined,
      });
      setArchive(null);
      if (fileRef.current) fileRef.current.value = '';
    } catch (e) {
      setError(toUserMessage(e, t('settings.slackImport.startFailed')));
    }
  };

  return (
    <div className="-mx-4 flex flex-col" data-qa="slack-import-panel">
      <div className="px-4 py-3 border-b border-line/40 space-y-2">
        <p className="text-xs text-muted">
          <Trans
            t={t}
            i18nKey="settings.slackImport.intro"
            components={{ slack: <span className="text-fg-dim" /> }}
          />
        </p>

        <input
          ref={fileRef}
          type="file"
          accept=".zip,application/zip"
          onChange={(e) => {
            const file = e.target.files?.[0] ?? null;
            setArchive(file);
            // Slack writes the workspace's name into the file name and nowhere
            // else, so that is where the suggestion comes from.
            if (file && !newName.trim()) setNewName(workspaceNameFrom(file.name));
          }}
          aria-label={t('settings.slackImport.file')}
          data-qa="slack-import-file"
          className="w-full text-xs text-fg-dim file:mr-2 file:px-2 file:py-1 file:rounded file:border-0 file:bg-raised file:text-fg-soft hover:file:bg-elevated file:cursor-pointer"
        />

        {mayCreate && (
          <Checkbox
            label={t('settings.slackImport.intoNew')}
            checked={intoNew}
            onChange={setIntoNew}
            dataQa="slack-import-into-new"
          />
        )}

        {intoNew && mayCreate && (
          <TextInput
            label={t('settings.slackImport.newName')}
            labelHidden
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder={t('settings.slackImport.newNamePlaceholder')}
            dataQa="slack-import-workspace-name"
          />
        )}

        <Checkbox
          label={t('settings.slackImport.dryRun')}
          description={t('settings.slackImport.dryRunHint')}
          checked={dryRun}
          onChange={setDryRun}
          dataQa="slack-import-dry-run"
        />

        {error && (
          <p className="text-2xs text-danger" data-qa="slack-import-form-error">
            {error}
          </p>
        )}

        <div className="flex justify-end">
          <Button
            variant="primary"
            size="sm"
            onClick={submit}
            disabled={start.isPending}
            data-qa="slack-import-start"
          >
            {start.isPending
              ? t('settings.slackImport.uploading')
              : dryRun
                ? t('settings.slackImport.check')
                : t('settings.slackImport.import')}
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex justify-center py-8">
            <Spinner className="text-accent" />
          </div>
        ) : runs.length === 0 ? (
          <div className="text-center py-10 px-6 text-sm text-muted" data-qa="slack-import-empty">
            {t('settings.slackImport.empty')}
          </div>
        ) : (
          runs.map((run) => (
            <RunRow
              key={run.id}
              run={run}
              retrying={retry.isPending && retry.variables === run.id}
              onRetry={() =>
                retry.mutate(run.id, {
                  onError: (e) => setError(toUserMessage(e, t('settings.slackImport.retryFailed'))),
                })
              }
              onDiscard={() => setDiscarding(run)}
            />
          ))
        )}
      </div>

      {discarding && (
        <ConfirmDialog
          title={t('settings.slackImport.discardTitle')}
          description={t('settings.slackImport.discardDescription', { source: discarding.source })}
          confirmLabel={t('settings.slackImport.discard')}
          onConfirm={async () => {
            try {
              await discard.mutateAsync(discarding.id);
            } catch (e) {
              throw new Error(toUserMessage(e, t('settings.slackImport.discardFailed')), { cause: e });
            }
          }}
          onClose={() => setDiscarding(null)}
          dataQa="slack-import-discard-dialog"
        />
      )}
    </div>
  );
}
