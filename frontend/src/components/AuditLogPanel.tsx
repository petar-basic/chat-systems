import { ScrollText } from 'lucide-react';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
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
      className="w-full lg:w-[32rem] max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="audit-log-panel"
    >
      <PanelHeader title="Audit log" icon={<ScrollText className="h-4 w-4 text-muted" />} onClose={onClose} />

      <div className="flex-1 overflow-y-auto p-4">
        {error ? (
          <div className="text-sm text-danger">
            {error instanceof Error ? error.message : 'Failed to load the audit log'}
          </div>
        ) : (
          <AuditLogTable entries={entries} loading={isLoading} />
        )}
      </div>
    </div>
  );
}
