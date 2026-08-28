import { formatDateTime } from '@/lib/datetime';
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
  'user.provisioned': 'User provisioned',
  'user.data_erased': 'User data erased',
  'channel.post_policy_changed': 'Posting policy changed',
  'message.history_read': 'Edit history read',
  'command.invoked': 'Command run',
  'emoji.created': 'Custom emoji added',
  'emoji.deleted': 'Custom emoji removed',
  'group.created': 'Group created',
  'group.updated': 'Group updated',
  'group.deleted': 'Group deleted',
  'group.member_added': 'Added to group',
  'group.member_removed': 'Removed from group',
  'export.requested': 'Export requested',
  'export.completed': 'Export completed',
  'retention.changed': 'Retention policy changed',
  'retention.purge': 'Retention purge',
  'scim.token_created': 'SCIM token created',
  'scim.token_revoked': 'SCIM token revoked',
  'scim.token_rotated': 'SCIM token rotated',
  'slack_import.started': 'Slack import started',
  'sso.linked': 'SSO account linked',
  'totp.enrolled': 'Two-factor enabled',
  'totp.disabled': 'Two-factor disabled',
  'totp.failed': 'Two-factor challenge failed',
  'totp.recovery_used': 'Recovery code used',
};

function humanizeAction(action: string): string {
  const words = action.replace(/[._]/g, ' ').trim();
  return words ? words.charAt(0).toUpperCase() + words.slice(1) : action;
}

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
      <div className="text-sm text-muted py-10 text-center" data-qa="audit-log-empty">
        Nothing has been recorded yet.
      </div>
    );
  }

  return (
    <div className="bg-surface rounded-xl border border-line overflow-x-auto" data-qa="audit-log-table">
      <table className="w-full min-w-[720px]">
        <thead>
          <tr className="border-b border-line">
            <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
              When
            </th>
            <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
              Who
            </th>
            <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
              Action
            </th>
            {showWorkspace && (
              <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                Workspace
              </th>
            )}
            <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
              Details
            </th>
            <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
              From
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {entries.map((entry) => (
            <tr key={entry.id} className="hover:bg-raised/30 transition" data-qa="audit-log-row">
              <td className="px-4 py-3 text-xs text-muted whitespace-nowrap">
                {formatDateTime(entry.created_at)}
              </td>
              <td className="px-4 py-3 text-sm text-fg">{actorLabel(entry)}</td>
              <td className="px-4 py-3 text-sm text-fg-soft" data-qa="audit-log-action">
                {ACTION_LABELS[entry.action] ?? humanizeAction(entry.action)}
              </td>
              {showWorkspace && (
                <td className="px-4 py-3 text-xs text-muted font-mono">
                  {entry.workspace_id?.slice(0, 8) ?? '—'}
                </td>
              )}
              <td className="px-4 py-3 text-xs text-muted max-w-md truncate">
                {detailSummary(entry.details) || '—'}
              </td>
              <td className="px-4 py-3 text-xs text-muted font-mono">{entry.ip_address ?? '—'}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
