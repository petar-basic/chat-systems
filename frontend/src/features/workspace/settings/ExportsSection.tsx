import { useState } from 'react';
import { AlertTriangle, CheckCircle2, Download, Loader2, PackageOpen } from 'lucide-react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { toUserMessage } from '@/lib/errors';
import { formatDateTime } from '@/lib/datetime';
import { getUserDisplay } from '@/lib/userHelpers';
import type { WorkspaceMember } from '@/stores/workspace';
import {
  useExports,
  useStartWorkspaceExport,
  type ExportDetail,
} from '@/features/workspace/hooks/useExports';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  isOwner: boolean;
  members: WorkspaceMember[];
}

function ExportRow({
  row,
  members,
  fetched,
  onFetch,
}: {
  row: ExportDetail;
  members: WorkspaceMember[];
  fetched: boolean;
  onFetch: () => void;
}) {
  const job = row.export;
  const running = job.status === 'pending' || job.status === 'running';
  const subject =
    job.scope === 'user' && job.subject_user_id
      ? getUserDisplay(job.subject_user_id, members).displayName
      : 'Whole workspace';

  return (
    <div className="flex items-center gap-3 px-4 py-3 border-b border-line/40" data-qa="export-row">
      {running && <Loader2 className="w-4 h-4 text-accent-soft animate-spin shrink-0" />}
      {job.status === 'complete' && <CheckCircle2 className="w-4 h-4 text-success shrink-0" />}
      {job.status === 'failed' && <AlertTriangle className="w-4 h-4 text-danger shrink-0" />}
      <div className="min-w-0 flex-1">
        <div className="text-sm text-fg-soft truncate" data-qa="export-subject">
          {subject}
          {job.include_dms && job.scope === 'workspace' && (
            <span className="ml-2 px-1.5 py-0.5 rounded bg-raised text-[10px] uppercase tracking-wide text-fg-dim">
              with DMs
            </span>
          )}
        </div>
        <div className="text-xs text-muted">
          {formatDateTime(job.created_at)} · <span data-qa="export-status">{job.status}</span>
          {job.error && <span className="text-danger"> · {job.error}</span>}
        </div>
      </div>
      {row.download_url && !fetched ? (
        <a
          href={row.download_url}
          download
          onClick={onFetch}
          data-qa="export-download"
          className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-lg bg-raised hover:bg-elevated text-fg transition"
        >
          <Download className="w-4 h-4" />
          Download
        </a>
      ) : (
        job.status === 'complete' && (
          <span className="text-xs text-muted" data-qa="export-link-gone">
            {fetched ? 'Downloaded' : 'Link used or expired'}
          </span>
        )
      )}
    </div>
  );
}

export function ExportsSection({ workspaceId, instanceUrl, isOwner, members }: Props) {
  const { data: rows = [], isLoading, error } = useExports(workspaceId, instanceUrl);
  const start = useStartWorkspaceExport(workspaceId, instanceUrl);
  const [includeDms, setIncludeDms] = useState(false);
  const [fetched, setFetched] = useState<Set<string>>(() => new Set());

  return (
    <div className="-mx-4 flex flex-col" data-qa="exports-section">
      <div className="px-4 pb-4 border-b border-line/50 space-y-3">
        <p className="text-sm text-muted">
          An export is a tar archive of the workspace in JSON lines. Its download link works once and expires
          after a day.
        </p>
        {isOwner ? (
          <>
            <Checkbox
              label="Include direct messages"
              description="Private conversations between members. Asking for them is recorded in the audit log."
              checked={includeDms}
              onChange={setIncludeDms}
              dataQa="export-include-dms"
            />
            <Button
              variant="primary"
              icon={<PackageOpen className="w-4 h-4" />}
              loading={start.isPending}
              onClick={() => start.mutate(includeDms)}
              data-qa="export-start"
            >
              Export workspace
            </Button>
            {start.isError && (
              <p role="alert" className="text-sm text-danger">
                {toUserMessage(start.error, 'Failed to start the export')}
              </p>
            )}
          </>
        ) : (
          <p className="text-sm text-muted">Only the workspace owner can export the whole workspace.</p>
        )}
      </div>

      {isLoading ? (
        <div className="flex justify-center py-8">
          <Spinner className="text-accent" />
        </div>
      ) : error ? (
        <p className="px-4 py-6 text-sm text-danger">{toUserMessage(error, 'Failed to load exports')}</p>
      ) : rows.length === 0 ? (
        <p className="px-4 py-6 text-sm text-muted text-center">No exports yet.</p>
      ) : (
        rows.map((row) => (
          <ExportRow
            key={row.export.id}
            row={row}
            members={members}
            fetched={fetched.has(row.export.id)}
            onFetch={() => setFetched((prev) => new Set(prev).add(row.export.id))}
          />
        ))
      )}
    </div>
  );
}
