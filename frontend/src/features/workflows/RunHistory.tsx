import { CheckCircle2, CircleDashed, XCircle } from 'lucide-react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { formatDateTime } from '@/lib/datetime';
import { stepLabel, type Step, type WorkflowRun } from './workflowModel';
import { useWorkflowRuns } from './useWorkflows';

interface Props {
  workflowId: string;
  steps: Step[];
  instanceUrl?: string;
}

const STATUS: Record<WorkflowRun['status'], { label: string; className: string }> = {
  pending: { label: 'Queued', className: 'text-muted' },
  running: { label: 'Running', className: 'text-accent' },
  succeeded: { label: 'Succeeded', className: 'text-success' },
  failed: { label: 'Failed', className: 'text-danger' },
  waiting: { label: 'Waiting for a form', className: 'text-accent' },
  expired: { label: 'Expired', className: 'text-muted' },
};

export function RunHistory({ workflowId, steps, instanceUrl }: Props) {
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
            {run.steps.length > 0 && (
              <ul className="mt-2 space-y-1">
                {run.steps.map((outcome) => {
                  const step = steps[outcome.position];
                  return (
                    <li
                      key={outcome.position}
                      className="flex items-center gap-1.5 text-xs text-fg-soft"
                      data-qa="workflow-run-step"
                    >
                      {outcome.succeeded ? (
                        <CheckCircle2 className="h-3.5 w-3.5 text-success" aria-label="Done" />
                      ) : (
                        <XCircle className="h-3.5 w-3.5 text-danger" aria-label="Failed" />
                      )}
                      Step {outcome.position + 1}
                      {step && ` · ${stepLabel(step)}`}
                    </li>
                  );
                })}
                {(run.status === 'running' || run.status === 'waiting') &&
                  steps.slice(run.steps.length).map((step, i) => (
                    <li key={`todo-${i}`} className="flex items-center gap-1.5 text-xs text-muted">
                      <CircleDashed className="h-3.5 w-3.5" aria-label="Waiting" />
                      Step {run.steps.length + i + 1} · {stepLabel(step)}
                    </li>
                  ))}
              </ul>
            )}
          </li>
        );
      })}
    </ol>
  );
}
