import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router';
import { UserX, UserCheck, ArrowLeft, RefreshCw } from 'lucide-react';
import { useInstanceStore } from '../stores/instances';
import { useCurrentUser } from '../hooks/queries/useAuth';
import {
  useAdminAuditLog,
  useAdminUsers,
  useSetInstanceRole,
  useSetUserStatus,
  type AdminUser,
} from '../hooks/queries/useAdmin';
import AuditLogTable from '../features/workspace/components/AuditLogTable';
import { toUserMessage } from '@/lib/errors';

type AdminTab = 'users' | 'audit';

function instanceLabel(url: string) {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

export default function InstanceAdminPage() {
  const navigate = useNavigate();
  const { data: currentUser } = useCurrentUser();
  const { instances, activeInstanceUrl } = useInstanceStore();
  const [tab, setTab] = useState<AdminTab>('users');

  const instance = instances.find((i) => i.url === activeInstanceUrl);
  const instanceUrl = activeInstanceUrl ?? null;
  const isAdmin = !!currentUser?.is_instance_admin;

  const usersQuery = useAdminUsers(isAdmin ? instanceUrl : null);
  const auditQuery = useAdminAuditLog(isAdmin && tab === 'audit' ? instanceUrl : null);
  const setRole = useSetInstanceRole(instanceUrl);
  const setStatus = useSetUserStatus(instanceUrl);

  useEffect(() => {
    if (!isAdmin) navigate('/app', { replace: true });
  }, [isAdmin, navigate]);

  const users = usersQuery.data ?? [];
  const entries = auditQuery.data ?? [];
  const loading = usersQuery.isLoading;
  const auditLoading = auditQuery.isLoading;
  const refreshing = usersQuery.isFetching || auditQuery.isFetching;
  const error = [usersQuery.error, auditQuery.error, setRole.error, setStatus.error].find((e) => e != null);

  const refresh = () => {
    if (tab === 'audit') void auditQuery.refetch();
    else void usersQuery.refetch();
  };

  const handleRoleChange = (user: AdminUser, isInstanceAdmin: boolean) => {
    if (user.id === currentUser?.id) return;
    setRole.mutate({ userId: user.id, isAdmin: isInstanceAdmin });
  };

  const handleToggleStatus = (user: AdminUser) => {
    if (user.id === currentUser?.id) return;
    setStatus.mutate({ userId: user.id, suspend: user.status === 'active' });
  };

  const roleBusy = (user: AdminUser) => setRole.isPending && setRole.variables?.userId === user.id;
  const statusBusy = (user: AdminUser) => setStatus.isPending && setStatus.variables?.userId === user.id;

  if (!currentUser?.is_instance_admin) return null;

  return (
    <div className="h-screen bg-app text-fg flex flex-col">
      <div className="border-b border-surface px-4 sm:px-6 py-4 flex flex-wrap items-center gap-3 sm:gap-4">
        <button
          onClick={() => navigate('/app')}
          className="p-2 text-muted hover:text-fg hover:bg-surface rounded-lg transition cursor-pointer"
        >
          <ArrowLeft className="w-5 h-5" />
        </button>
        <div className="flex-1 min-w-0">
          <h1 className="text-lg font-bold truncate">Instance Admin</h1>
          {instance && <p className="text-xs text-muted truncate">{instanceLabel(instance.url)}</p>}
        </div>
        <div className="flex items-center gap-1 bg-surface rounded-lg p-1 shrink-0">
          {(['users', 'audit'] as const).map((value) => (
            <button
              key={value}
              data-qa={`admin-tab-${value}`}
              onClick={() => setTab(value)}
              className={`px-3 py-1.5 text-sm rounded-md transition cursor-pointer ${
                tab === value ? 'bg-raised text-fg' : 'text-muted hover:text-fg'
              }`}
            >
              <span className="whitespace-nowrap">{value === 'users' ? 'Users' : 'Audit log'}</span>
            </button>
          ))}
        </div>
        <button
          onClick={refresh}
          disabled={refreshing}
          className="flex items-center gap-2 px-3 py-2 text-sm text-muted hover:text-fg hover:bg-surface rounded-lg transition cursor-pointer disabled:opacity-50 shrink-0"
        >
          <RefreshCw className={`w-4 h-4 ${refreshing ? 'animate-spin' : ''}`} />
          <span className="max-sm:sr-only">Refresh</span>
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-4 sm:p-6">
        {error && (
          <div className="bg-red-500/10 border border-red-500/30 text-danger px-4 py-3 rounded-lg mb-4 text-sm">
            {toUserMessage(error)}
          </div>
        )}

        {tab === 'audit' ? (
          <div className="max-w-5xl mx-auto">
            <AuditLogTable entries={entries} loading={auditLoading} showWorkspace />
          </div>
        ) : loading ? (
          <div className="flex items-center justify-center py-20">
            <div className="w-8 h-8 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : (
          <div className="max-w-4xl mx-auto">
            <div className="mb-4 text-sm text-muted">{users.length} users on this instance</div>

            <div className="bg-surface rounded-xl border border-line overflow-x-auto">
              <table className="w-full min-w-[560px]">
                <thead>
                  <tr className="border-b border-line">
                    <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                      User
                    </th>
                    <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                      Status
                    </th>
                    <th className="text-left px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                      Role
                    </th>
                    <th className="text-right px-4 py-3 text-xs font-medium text-muted uppercase tracking-wider">
                      Actions
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
                            {user.display_name || '(no name)'}
                            {isSelf && <span className="ml-2 text-xs text-accent">(you)</span>}
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
                            {user.status}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          {roleBusy(user) ? (
                            <div className="w-4 h-4 border border-purple-400/30 border-t-purple-400 rounded-full animate-spin" />
                          ) : (
                            <select
                              aria-label="Instance role"
                              value={user.is_instance_admin ? 'admin' : 'user'}
                              onChange={(e) => handleRoleChange(user, e.target.value === 'admin')}
                              disabled={isSelf}
                              className="text-xs bg-raised border border-line-strong rounded px-2 py-1 text-fg focus:outline-none focus:ring-1 focus:ring-purple-500 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
                            >
                              <option value="user">User</option>
                              <option value="admin">Instance Admin</option>
                            </select>
                          )}
                        </td>
                        <td className="px-4 py-3">
                          <div className="flex items-center justify-end gap-2">
                            <button
                              onClick={() => handleToggleStatus(user)}
                              disabled={isSelf || statusBusy(user) || user.status === 'pending'}
                              className="flex items-center gap-1 px-2 py-1.5 rounded text-xs transition cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed text-muted hover:text-danger hover:bg-raised"
                              title={user.status === 'active' ? 'Suspend user' : 'Activate user'}
                            >
                              {statusBusy(user) ? (
                                <div className="w-3 h-3 border border-current border-t-transparent rounded-full animate-spin" />
                              ) : user.status === 'active' ? (
                                <UserX className="w-3.5 h-3.5" />
                              ) : (
                                <UserCheck className="w-3.5 h-3.5" />
                              )}
                              {user.status === 'active' ? 'Suspend' : 'Activate'}
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
