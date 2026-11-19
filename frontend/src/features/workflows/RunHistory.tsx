import { CheckCircle2, CircleDashed, XCircle } from 'lucide-react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { formatDateTime } from '@/lib/datetime';
import type { WorkflowRun } from './workflowModel';
import { useWorkflowRuns } from './useWorkflows';

interface Props {
  workflowId: string;
  instanceUrl?: string;
}

const STATUS: Record<WorkflowRun['status'], { label: string; className: string }> = {
  pending: { label: 'Queued', className: 'text-muted' },
  running: { label: 'Running', className: 'text-accent' },
  succeeded: { label: 'Succeeded', className: 'text-success' },
  failed: { label: 'Failed', className: 'text-danger' },
  waiting: { label: 'Waiting', className: 'text-accent' },
  expired: { label: 'Expired', className: 'text-muted' },
};

export function RunHistory({ workflowId, instanceUrl }: Props) {
  const runs = useWorkflowRuns(workflowId, instanceUrl);

  if (runs.isLoading) {
    return (
      <div className="flex justify-center py-8 text-accent">
        <Spinner />
      </div>
    );
  }
  const history = runs.data ?? [];
  if (history.length === 0) {
    return (
      <p className="py-8 text-center text-sm text-muted" data-qa="workflow-runs-empty">
        It hasn&apos;t run yet.
      </p>
    );
  }

  return (
    <ol className="space-y-3 py-3" aria-label="Run history" data-qa="workflow-runs">
      {history.map((run) => {
        const status = STATUS[run.status];
        return (
          <li key={run.id} className="rounded-lg border border-line p-3" data-qa="workflow-run">
            <div className="flex items-center gap-2 text-sm">
              <span className={`font-medium ${status.className}`} data-qa="workflow-run-status">
                {status.label}
              </span>
              <time dateTime={run.started_at} className="text-xs text-muted">
                {formatDateTime(run.started_at)}
              </time>
            </div>
            {run.error && (
              <p className="mt-1 text-sm text-danger" data-qa="workflow-run-error">
                {run.error}
              </p>
            )}
            {(run.steps.length > 0 || run.waiting) && (
              <ul className="mt-2 space-y-1">
                {run.steps.map((outcome) => (
                  <li key={outcome.position} className="text-xs text-fg-soft" data-qa="workflow-run-step">
                    <span className="flex items-center gap-1.5">
                      {outcome.succeeded ? (
                        <CheckCircle2 className="h-3.5 w-3.5 text-success" aria-label="Done" />
                      ) : (
                        <XCircle className="h-3.5 w-3.5 text-danger" aria-label="Failed" />
                      )}
                      Step {outcome.position + 1} · {outcome.label}
                      {outcome.matched != null && (
                        <span className="text-muted" data-qa="workflow-run-path">
                          — took the {outcome.matched ? '“If it matches”' : '“Otherwise”'} path
                        </span>
                      )}
                    </span>
                    {outcome.error && (
                      <span className="ml-5 block text-danger" data-qa="workflow-run-step-error">
                        {outcome.error}
                      </span>
                    )}
                    {outcome.inputs && (
                      <Handed label="Gave it" values={outcome.inputs} dataQa="workflow-run-inputs" />
                    )}
                    {outcome.outputs && (
                      <Handed label="Got back" values={outcome.outputs} dataQa="workflow-run-outputs" />
                    )}
                  </li>
                ))}
                {run.waiting && (
                  <li
                    className="flex items-center gap-1.5 text-xs text-accent"
                    data-qa="workflow-run-waiting"
                  >
                    <CircleDashed className="h-3.5 w-3.5" aria-label="Waiting" />
                    Waiting at step {run.waiting.position + 1} · {run.waiting.label}
                    {run.waiting.until && (
                      <>
                        {' '}
                        until <time dateTime={run.waiting.until}>{formatDateTime(run.waiting.until)}</time>
                      </>
                    )}
                  </li>
                )}
              </ul>
            )}
          </li>
        );
      })}
    </ol>
  );
}

function shown(value: unknown): string {
  if (value === null || value === undefined || value === '') return '—';
  return typeof value === 'string' ? value : JSON.stringify(value);
}

function Handed({ label, values, dataQa }: { label: string; values: object; dataQa: string }) {
  const entries = Object.entries(values);
  if (entries.length === 0) return null;
  return (
    <span className="ml-5 block text-muted" data-qa={dataQa}>
      {label}:{' '}
      {entries.map(([name, value], index) => (
        <span key={name}>
          {index > 0 && ', '}
          <span className="font-mono">{name}</span> {shown(value)}
        </span>
      ))}
    </span>
  );
}
