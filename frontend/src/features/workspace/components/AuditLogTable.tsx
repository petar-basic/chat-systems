import type { components } from '@/api/schema';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { formatDateTime } from '@/lib/datetime';
import { useTranslation } from 'react-i18next';

export type AuditEntry = components['schemas']['AuditRow'];

interface Props {
  entries: AuditEntry[];
  loading: boolean;
  showWorkspace?: boolean;
}

const ACTIONS = [
  'message.deleted',
  'channel.created',
  'channel.archived',
  'channel.unarchived',
  'channel.updated',
  'channel.member_added',
  'channel.member_removed',
  'channel.role_changed',
  'workspace.created',
  'workspace.deleted',
  'workspace.restored',
  'workspace.member_removed',
  'workspace.role_changed',
  'guest.updated',
  'guest.expired',
  'invite.created',
  'invite.revoked',
  'invite_request.created',
  'invite_request.approved',
  'invite_request.declined',
  'invite_request.expired',
  'profile_field.created',
  'profile_field.updated',
  'profile_field.deleted',
  'custom_response.created',
  'custom_response.updated',
  'custom_response.deleted',
  'workspace_bot.renamed',
  'hook.created',
  'app.configured',
  'app.removed',
  'app.connected',
  'app.disconnected',
  'bot_app.created',
  'bot_app.updated',
  'bot_app.token_rotated',
  'bot_app.removed',
  'bot_app.called',
  'bot_app.interaction',
  'bot_app.disabled',
  'bot_app.enabled',
  'workflow.created',
  'workflow.updated',
  'workflow.enabled',
  'workflow.disabled',
  'workflow.taken_over',
  'workflow.link_rotated',
  'workflow.deleted',
  'hook.deleted',
  'hook.revealed',
  'hook.rotated',
  'hook.disabled',
  'hook.enabled',
  'file.deleted',
  'user.suspended',
  'user.activated',
  'user.instance_role_changed',
  'user.provisioned',
  'user.data_erased',
  'channel.post_policy_changed',
  'channel.broadcast_mentions_changed',
  'channel.privacy_changed',
  'conversation.converted',
  'message.history_read',
  'command.invoked',
  'emoji.created',
  'emoji.deleted',
  'group.created',
  'group.updated',
  'group.deleted',
  'group.member_added',
  'group.member_removed',
  'export.requested',
  'export.completed',
  'retention.changed',
  'retention.purge',
  'scim.token_created',
  'scim.token_revoked',
  'scim.token_rotated',
  'slack_import.started',
  'slack_import.discarded',
  'sso.linked',
  'totp.enrolled',
  'totp.disabled',
  'totp.failed',
  'totp.recovery_used',
  'passkey.added',
  'passkey.removed',
  'passkey.used',
  'email_code.sent',
  'email_code.used',
  'email_code.exhausted',
  'instance.settings_changed',
] as const;

type Action = (typeof ACTIONS)[number];

const isAction = (action: string): action is Action => (ACTIONS as readonly string[]).includes(action);

function humanizeAction(action: string): string {
  const words = action.replace(/[._]/g, ' ').trim();
  return words ? words.charAt(0).toUpperCase() + words.slice(1) : action;
}

function actorLabel(entry: AuditEntry) {
  return entry.actor_display_name || entry.actor_email || entry.user_id;
}

function detailSummary(details: unknown) {
  if (!details || typeof details !== 'object') return '';
  const parts = Object.entries(details)
    .filter(([, value]) => value !== null && value !== undefined && value !== '')
    .map(([key, value]) => `${key}: ${typeof value === 'object' ? JSON.stringify(value) : String(value)}`);
  return parts.join(' · ');
}

export default function AuditLogTable({ entries, loading, showWorkspace = false }: Props) {
  const { t } = useTranslation('admin');

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Spinner size="lg" className="text-accent" />
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <div className="text-sm text-muted py-10 text-center" data-qa="audit-log-empty">
        {t('audit.empty')}
      </div>
    );
  }

  return (
    <div className="bg-surface rounded-xl border border-line overflow-x-auto" data-qa="audit-log-table">
      <table className="w-full min-w-180">
        <thead>
          <tr className="border-b border-line">
            <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
              {t('audit.when')}
            </th>
            <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
              {t('audit.who')}
            </th>
            <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
              {t('audit.action')}
            </th>
            {showWorkspace && (
              <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                {t('audit.workspace')}
              </th>
            )}
            <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
              {t('audit.details')}
            </th>
            <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
              {t('audit.from')}
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {entries.map((entry) => (
            <tr key={entry.id} className="hover:bg-raised/30 transition" data-qa="audit-log-row">
              <td className="px-4 py-3 text-xs text-muted whitespace-nowrap">
                {formatDateTime(entry.created_at)}
              </td>
              <td className="px-4 py-3 text-sm text-fg">{actorLabel(entry) || t('audit.unknownActor')}</td>
              <td className="px-4 py-3 text-sm text-fg-soft" data-qa="audit-log-action">
                {isAction(entry.action) ? t(`audit.actions.${entry.action}`) : humanizeAction(entry.action)}
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
