export interface AuditEntry {
  id: string;
  workspace_id: string | null;
  user_id: string | null;
  actor_email: string | null;
  actor_display_name: string | null;
  action: string;
  resource_type: string | null;
  resource_id: string | null;
  details: Record<string, unknown>;
  ip_address: string | null;
  created_at: string;
}

interface Props {
  entries: AuditEntry[];
  loading: boolean;
  showWorkspace?: boolean;
}

const ACTION_LABELS: Record<string, string> = {
  'message.deleted': 'Message deleted',
  'channel.created': 'Channel created',
  'channel.archived': 'Channel archived',
  'channel.updated': 'Channel updated',
  'channel.member_added': 'Added to channel',
  'channel.member_removed': 'Removed from channel',
  'channel.role_changed': 'Channel role changed',
  'workspace.created': 'Workspace created',
  'workspace.deleted': 'Workspace deleted',
  'workspace.restored': 'Workspace restored',
  'workspace.member_removed': 'Member removed',
  'workspace.role_changed': 'Role changed',
  'invite.created': 'Invite created',
  'invite.revoked': 'Invite revoked',
  'hook.created': 'Integration created',
  'hook.deleted': 'Integration deleted',
  'hook.revealed': 'Integration secret revealed',
  'hook.rotated': 'Integration secret rotated',
  'file.deleted': 'File deleted',
  'user.suspended': 'User suspended',
  'user.activated': 'User activated',
  'user.instance_role_changed': 'Instance role changed',
};

function actorLabel(entry: AuditEntry) {
  return entry.actor_display_name || entry.actor_email || entry.user_id || 'unknown';
}

function detailSummary(details: Record<string, unknown>) {
  const parts = Object.entries(details)
    .filter(([, value]) => value !== null && value !== undefined && value !== '')
    .map(([key, value]) => `${key}: ${typeof value === 'object' ? JSON.stringify(value) : String(value)}`);
  return parts.join(' · ');
}

export default function AuditLogTable({ entries, loading, showWorkspace = false }: Props) {
  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-8 h-8 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <div className="text-sm text-slate-400 py-10 text-center" data-qa="audit-log-empty">
        Nothing has been recorded yet.
      </div>
    );
  }

  return (
    <div
      className="bg-slate-800 rounded-xl border border-slate-700 overflow-x-auto"
      data-qa="audit-log-table"
    >
      <table className="w-full min-w-[720px]">
        <thead>
          <tr className="border-b border-slate-700">
            <th className="text-left px-4 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">
              When
            </th>
            <th className="text-left px-4 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">
              Who
            </th>
            <th className="text-left px-4 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">
              Action
            </th>
            {showWorkspace && (
              <th className="text-left px-4 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">
                Workspace
              </th>
            )}
            <th className="text-left px-4 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">
              Details
            </th>
            <th className="text-left px-4 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">
              From
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-slate-700">
          {entries.map((entry) => (
            <tr key={entry.id} className="hover:bg-slate-700/30 transition" data-qa="audit-log-row">
              <td className="px-4 py-3 text-xs text-slate-400 whitespace-nowrap">
                {new Date(entry.created_at).toLocaleString()}
              </td>
              <td className="px-4 py-3 text-sm text-white">{actorLabel(entry)}</td>
              <td className="px-4 py-3 text-sm text-slate-200" data-qa="audit-log-action">
                {ACTION_LABELS[entry.action] ?? entry.action}
              </td>
              {showWorkspace && (
                <td className="px-4 py-3 text-xs text-slate-400 font-mono">
                  {entry.workspace_id?.slice(0, 8) ?? '—'}
                </td>
              )}
              <td className="px-4 py-3 text-xs text-slate-400 max-w-md truncate">
                {detailSummary(entry.details) || '—'}
              </td>
              <td className="px-4 py-3 text-xs text-slate-400 font-mono">{entry.ip_address ?? '—'}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
