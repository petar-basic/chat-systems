import { X, ScrollText } from 'lucide-react';
import { useAuditLog } from '@/hooks/queries/useAuditLog';
import AuditLogTable from '@/features/workspace/components/AuditLogTable';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  onClose: () => void;
}

export default function AuditLogPanel({ workspaceId, instanceUrl, onClose }: Props) {
  const { data: entries = [], isLoading, error } = useAuditLog(workspaceId, instanceUrl);

  return (
    <div
      className="w-full lg:w-[32rem] max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-slate-800 border-l border-slate-700/50 flex flex-col"
      data-qa="audit-log-panel"
    >
      <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700/50 shrink-0">
        <div className="flex items-center gap-2">
          <ScrollText className="w-4 h-4 text-slate-400" />
          <span className="font-semibold">Audit log</span>
        </div>
        <button onClick={onClose} className="text-slate-400 hover:text-white transition cursor-pointer">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        {error ? (
          <div className="text-sm text-red-400">
            {error instanceof Error ? error.message : 'Failed to load the audit log'}
          </div>
        ) : (
          <AuditLogTable entries={entries} loading={isLoading} />
        )}
      </div>
    </div>
  );
}
