import { useAuditLog } from '@/features/workspace/hooks/useAuditLog';
import AuditLogTable from '@/features/workspace/components/AuditLogTable';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
}

export function AuditLogSection({ workspaceId, instanceUrl }: Props) {
  const { data: entries = [], isLoading, error } = useAuditLog(workspaceId, instanceUrl);

  return (
    <div className="-mx-4 flex flex-col" data-qa="audit-log-panel">
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
