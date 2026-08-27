import { useRef, useState } from 'react';
import { X, Upload, CheckCircle2, AlertTriangle, Loader2 } from 'lucide-react';
import {
  useSlackImports,
  useStartSlackImport,
  workspaceNameFrom,
  type SlackImportRun,
} from '@/hooks/queries/useSlackImport';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  onClose: () => void;
}

const COUNTS: { key: keyof NonNullable<SlackImportRun['report']> & string; label: string }[] = [
  { key: 'messages_imported', label: 'messages' },
  { key: 'channels_created', label: 'channels' },
  { key: 'conversations_created', label: 'DMs' },
  { key: 'users_created', label: 'accounts created' },
  { key: 'users_matched', label: 'accounts matched' },
  { key: 'threads_resolved', label: 'in threads' },
  { key: 'reactions', label: 'reactions' },
  { key: 'files_imported', label: 'files' },
  { key: 'emoji_imported', label: 'emoji' },
];

function RunRow({ run }: { run: SlackImportRun }) {
  const report = (run.report ?? {}) as Record<string, number | undefined>;
  const skipped = (run.report as SlackImportRun['report'])?.skipped ?? [];
  const notes = (run.report as SlackImportRun['report'])?.notes ?? [];
  const running = run.status === 'pending' || run.status === 'running';

  return (
    <div className="px-4 py-3 border-b border-slate-700/40" data-qa="slack-import-run">
      <div className="flex items-center gap-2">
        {running && <Loader2 className="w-3.5 h-3.5 text-purple-300 animate-spin shrink-0" />}
        {run.status === 'complete' && <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400 shrink-0" />}
        {run.status === 'failed' && <AlertTriangle className="w-3.5 h-3.5 text-red-400 shrink-0" />}
        <span className="text-sm text-slate-200 truncate" data-qa="slack-import-source">
          {run.source}
        </span>
        {run.dry_run && (
          <span className="px-1.5 py-0.5 rounded bg-slate-700 text-[10px] uppercase tracking-wide text-slate-300">
            dry run
          </span>
        )}
        <span className="ml-auto text-xs text-slate-400" data-qa="slack-import-status">
          {run.status}
        </span>
      </div>

      <div className="mt-1.5 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-slate-400">
        {COUNTS.filter(({ key }) => (report[key] ?? 0) > 0).map(({ key, label }) => (
          <span key={key}>
            <span className="text-slate-200 tabular-nums">{report[key]}</span> {label}
          </span>
        ))}
      </div>

      {notes.length > 0 && (
        <ul className="mt-1.5 space-y-0.5" data-qa="slack-import-notes">
          {notes.map((note, i) => (
            <li key={i} className="text-[11px] text-slate-400">
              {note}
            </li>
          ))}
        </ul>
      )}

      {run.error && (
        <p className="mt-1.5 text-xs text-red-400" data-qa="slack-import-error">
          {run.error}
        </p>
      )}

      {skipped.length > 0 && (
        <details className="mt-1.5">
          <summary className="text-xs text-slate-400 cursor-pointer hover:text-slate-200">
            {skipped.length} not imported
          </summary>
          <ul className="mt-1 space-y-0.5" data-qa="slack-import-skipped">
            {skipped.slice(0, 50).map((item, i) => (
              <li key={i} className="text-[11px] text-slate-400">
                <span className="text-slate-300">{item.what}</span> — {item.why}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}

export default function SlackImportPanel({ workspaceId, instanceUrl, onClose }: Props) {
  const { data: runs = [], isLoading } = useSlackImports(workspaceId, instanceUrl);
  const start = useStartSlackImport(workspaceId, instanceUrl);
  const fileRef = useRef<HTMLInputElement>(null);

  const [archive, setArchive] = useState<File | null>(null);
  const [dryRun, setDryRun] = useState(true);
  const [intoNew, setIntoNew] = useState(false);
  const [newName, setNewName] = useState('');
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    if (!archive) {
      setError('Choose the .zip Slack gave you');
      return;
    }
    if (intoNew && !newName.trim()) {
      setError('Name the workspace to import into');
      return;
    }
    setError(null);
    try {
      await start.mutateAsync({
        archive,
        dryRun,
        newWorkspaceName: intoNew ? newName.trim() : undefined,
      });
      setArchive(null);
      if (fileRef.current) fileRef.current.value = '';
    } catch (e) {
      setError((e as { message?: string })?.message || 'Failed to start that import');
    }
  };

  return (
    <div
      className="w-full lg:w-96 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-slate-800 border-l border-slate-700/50 flex flex-col"
      data-qa="slack-import-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700/50 shrink-0">
        <div className="flex items-center gap-2">
          <Upload className="w-4 h-4 text-slate-400" />
          <span className="font-semibold">Slack import</span>
        </div>
        <button
          onClick={onClose}
          aria-label="Close Slack import"
          className="text-slate-400 hover:text-white transition cursor-pointer"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="px-4 py-3 border-b border-slate-700/40 space-y-2">
        <p className="text-xs text-slate-400">
          The .zip from Slack&apos;s <span className="text-slate-300">Export workspace data</span>. Imported
          messages keep their original dates, so nobody is notified about a conversation from two years ago.
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
          aria-label="Slack export"
          data-qa="slack-import-file"
          className="w-full text-xs text-slate-300 file:mr-2 file:px-2 file:py-1 file:rounded file:border-0 file:bg-slate-700 file:text-slate-200 hover:file:bg-slate-600 file:cursor-pointer"
        />

        <label className="flex items-center gap-2 text-xs text-slate-300">
          <input
            type="checkbox"
            checked={intoNew}
            onChange={(e) => setIntoNew(e.target.checked)}
            data-qa="slack-import-into-new"
            className="accent-purple-500"
          />
          Import into a new workspace
        </label>

        {intoNew && (
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Workspace name"
            aria-label="New workspace name"
            data-qa="slack-import-workspace-name"
            className="w-full px-2 py-1.5 bg-slate-700/50 border border-slate-600 rounded text-sm text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
        )}

        <label className="flex items-center gap-2 text-xs text-slate-300">
          <input
            type="checkbox"
            checked={dryRun}
            onChange={(e) => setDryRun(e.target.checked)}
            data-qa="slack-import-dry-run"
            className="accent-purple-500"
          />
          Dry run — report what would happen, write nothing
        </label>

        {error && (
          <p className="text-[11px] text-red-400" data-qa="slack-import-form-error">
            {error}
          </p>
        )}

        <div className="flex justify-end">
          <button
            onClick={submit}
            disabled={start.isPending}
            data-qa="slack-import-start"
            className="px-3 py-1 text-xs bg-purple-600 hover:bg-purple-500 text-white rounded transition cursor-pointer disabled:opacity-50"
          >
            {start.isPending ? 'Uploading…' : dryRun ? 'Check the export' : 'Import'}
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : runs.length === 0 ? (
          <div className="text-center py-10 px-6 text-sm text-slate-400" data-qa="slack-import-empty">
            No imports yet. A dry run first is the cheap way to see what will not convert.
          </div>
        ) : (
          runs.map((run) => <RunRow key={run.id} run={run} />)
        )}
      </div>
    </div>
  );
}
