import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { useNavigate } from 'react-router';
import { UserX, UserCheck, ArrowLeft, RefreshCw } from 'lucide-react';
import { useInstanceStore } from '@/stores/instances';
import { useCurrentUser } from '@/features/instance/hooks/useAuth';
import {
  useAdminAuditLog,
  useAdminWorkspaces,
  useAdminUsers,
  useSetInstanceRole,
  useSetUserStatus,
  type AdminUser,
  type InstanceRole,
} from '@/features/instance/hooks/useAdmin';
import AuditLogTable from '@/features/workspace/components/AuditLogTable';
import { toUserMessage } from '@/lib/errors';
import { Select } from '@/shared/components/Form/Select';
import { TestEmailCheck } from '@/features/setup/TestEmailCheck';
import { EmailCodeSetting } from '@/features/setup/EmailCodeSetting';
import { WorkspaceCreationSetting } from '@/features/setup/WorkspaceCreationSetting';
import { AdminWorkspaces } from '@/features/instance/AdminWorkspaces';

type AdminTab = 'users' | 'workspaces' | 'audit' | 'email';

const INSTANCE_ROLES: readonly InstanceRole[] = ['member', 'workspace_creator', 'admin'];

function isInstanceRole(value: string): value is InstanceRole {
  return (INSTANCE_ROLES as readonly string[]).includes(value);
}

function instanceLabel(url: string) {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

export default function InstanceAdminPage() {
  const { t } = useTranslation(['admin', 'common']);
  const navigate = useNavigate();
  const { data: currentUser } = useCurrentUser();
  const { instances, activeInstanceUrl } = useInstanceStore();
  const [tab, setTab] = useState<AdminTab>('users');

  const instance = instances.find((i) => i.url === activeInstanceUrl);
  const instanceUrl = activeInstanceUrl ?? null;
  const isAdmin = currentUser?.instance_role === 'admin';

  const usersQuery = useAdminUsers(isAdmin ? instanceUrl : null);
  const auditQuery = useAdminAuditLog(isAdmin && tab === 'audit' ? instanceUrl : null);
  const workspacesQuery = useAdminWorkspaces(isAdmin && tab === 'workspaces' ? instanceUrl : null);
  const setRole = useSetInstanceRole(instanceUrl);
  const setStatus = useSetUserStatus(instanceUrl);

  useEffect(() => {
    if (!isAdmin) navigate('/app', { replace: true });
  }, [isAdmin, navigate]);

  const users = usersQuery.data ?? [];
  const entries = auditQuery.data ?? [];
  const loading = usersQuery.isLoading;
  const auditLoading = auditQuery.isLoading;
  const refreshing = usersQuery.isFetching || auditQuery.isFetching || workspacesQuery.isFetching;
  const error = [usersQuery.error, auditQuery.error, setRole.error, setStatus.error].find((e) => e != null);

  const refresh = () => {
    if (tab === 'audit') void auditQuery.refetch();
    else if (tab === 'workspaces') void workspacesQuery.refetch();
    else void usersQuery.refetch();
  };

  const handleRoleChange = (user: AdminUser, role: string) => {
    if (user.id === currentUser?.id || !isInstanceRole(role) || role === user.instance_role) return;
    setRole.mutate({ userId: user.id, role });
  };

  const handleToggleStatus = (user: AdminUser) => {
    if (user.id === currentUser?.id) return;
    setStatus.mutate({ userId: user.id, suspend: user.status === 'active' });
  };

  const roleBusy = (user: AdminUser) => setRole.isPending && setRole.variables?.userId === user.id;
  const statusBusy = (user: AdminUser) => setStatus.isPending && setStatus.variables?.userId === user.id;

  if (!isAdmin) return null;

  return (
    <div className="flex-1 min-h-0 bg-app text-fg flex flex-col">
      <div className="border-b border-surface px-4 sm:px-6 py-4 flex flex-wrap items-center gap-3 sm:gap-4">
        <button
          onClick={() => navigate('/app')}
          aria-label={t('page.back')}
          data-tooltip={t('page.back')}
          className="p-2 text-muted hover:text-fg hover:bg-surface rounded-lg transition cursor-pointer"
        >
          <ArrowLeft className="w-5 h-5" />
        </button>
        <div className="flex-1 min-w-0">
          <h1 className="text-lg font-bold truncate">{t('page.title')}</h1>
          {instance && <p className="text-xs text-muted truncate">{instanceLabel(instance.url)}</p>}
        </div>
        <div className="flex items-center gap-1 bg-surface rounded-lg p-1 shrink-0">
          {(['users', 'workspaces', 'audit', 'email'] as const).map((value) => (
            <button
              key={value}
              data-qa={`admin-tab-${value}`}
              onClick={() => setTab(value)}
              className={`px-3 py-1.5 text-sm rounded-md transition cursor-pointer ${
                tab === value ? 'bg-raised text-fg' : 'text-muted hover:text-fg'
              }`}
            >
              <span className="whitespace-nowrap">{t(`page.tabs.${value}`)}</span>
            </button>
          ))}
        </div>
        <button
          onClick={refresh}
          disabled={refreshing}
          className="flex items-center gap-2 px-3 py-2 text-sm text-muted hover:text-fg hover:bg-surface rounded-lg transition cursor-pointer disabled:opacity-50 shrink-0"
        >
          <RefreshCw className={`w-4 h-4 ${refreshing ? 'animate-spin' : ''}`} />
          <span className="max-sm:sr-only">{t('page.refresh')}</span>
        </button>
      </div>

      <div className="relative flex-1 overflow-y-auto p-4 sm:p-6">
        {error && (
          <div className="bg-red-500/10 border border-red-500/30 text-danger px-4 py-3 rounded-lg mb-4 text-sm">
            {toUserMessage(error)}
          </div>
        )}

        {tab === 'workspaces' ? (
          <AdminWorkspaces instanceUrl={instanceUrl} />
        ) : tab === 'email' ? (
          <div className="max-w-2xl mx-auto bg-surface rounded-xl border border-line p-6 space-y-6">
            <TestEmailCheck instanceUrl={instanceUrl} />
            <EmailCodeSetting instanceUrl={instanceUrl} />
          </div>
        ) : tab === 'audit' ? (
          <div className="max-w-5xl mx-auto">
            <AuditLogTable entries={entries} loading={auditLoading} showWorkspace />
          </div>
        ) : loading ? (
          <div className="flex items-center justify-center py-20">
            <Spinner size="lg" className="text-accent" />
          </div>
        ) : (
          <div className="max-w-4xl mx-auto">
            <div className="mb-6 bg-surface rounded-xl border border-line p-4 sm:p-6">
              <WorkspaceCreationSetting instanceUrl={instanceUrl} />
            </div>
            <div className="mb-4 text-sm text-muted">{t('users.count', { count: users.length })}</div>

            <div className="bg-surface rounded-xl border border-line overflow-x-auto">
              <table className="w-full min-w-140">
                <thead>
                  <tr className="border-b border-line">
                    <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                      {t('users.user')}
                    </th>
                    <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                      {t('users.status')}
                    </th>
                    <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                      {t('users.role')}
                    </th>
                    <th className="text-right px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                      {t('common:actions')}
                    </th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-line">
                  {users.map((user) => {
                    const isSelf = user.id === currentUser?.id;
                    return (
                      <tr key={user.id} className="hover:bg-raised/30 transition">
                        <td className="px-4 py-3">
                          <div className="font-medium text-sm text-fg">
                            {user.display_name || t('users.noName')}
                            {isSelf && <span className="ml-2 text-xs text-accent">{t('users.you')}</span>}
                          </div>
                          <div className="text-xs text-muted">{user.email}</div>
                        </td>
                        <td className="px-4 py-3">
                          <span
                            className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                              user.status === 'active'
                                ? 'bg-green-500/10 text-success'
                                : user.status === 'pending'
                                  ? 'bg-yellow-500/10 text-warning'
                                  : 'bg-red-500/10 text-danger'
                            }`}
                          >
                            {t(`common:userStatus.${user.status}`, { defaultValue: user.status })}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          {roleBusy(user) ? (
                            <Spinner size="sm" className="text-accent" />
                          ) : (
                            <Select
                              label={t('users.roleFor', { name: user.display_name || user.email })}
                              labelHidden
                              compact
                              value={user.instance_role}
                              onChange={(e) => handleRoleChange(user, e.target.value)}
                              disabled={isSelf}
                              className="w-44"
                              dataQa="instance-role-select"
                            >
                              {INSTANCE_ROLES.map((role) => (
                                <option key={role} value={role}>
                                  {t(`instanceRoles.${role}`)}
                                </option>
                              ))}
                            </Select>
                          )}
                        </td>
                        <td className="px-4 py-3">
                          <div className="flex items-center justify-end gap-2">
                            <button
                              onClick={() => handleToggleStatus(user)}
                              disabled={isSelf || statusBusy(user) || user.status === 'pending'}
                              className="flex items-center gap-1 px-2 py-1.5 rounded text-xs transition cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed text-muted hover:text-danger hover:bg-raised"
                              data-tooltip={
                                user.status === 'active' ? t('users.suspendHint') : t('users.activateHint')
                              }
                            >
                              {statusBusy(user) ? (
                                <Spinner size="sm" />
                              ) : user.status === 'active' ? (
                                <UserX className="w-3.5 h-3.5" />
                              ) : (
                                <UserCheck className="w-3.5 h-3.5" />
                              )}
                              {user.status === 'active' ? t('users.suspend') : t('users.activate')}
                            </button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
