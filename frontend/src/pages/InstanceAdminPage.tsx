import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router';
import { UserX, UserCheck, ArrowLeft, RefreshCw } from 'lucide-react';
import { useInstanceStore } from '../stores/instances';
import { instanceManager } from '../lib/instances';
import { useCurrentUser } from '../hooks/queries/useAuth';
import AuditLogTable, { type AuditEntry } from '../features/workspace/components/AuditLogTable';
import type { components } from '@/api/schema';

type AdminUser = components['schemas']['AdminUser'];

type AdminTab = 'users' | 'audit';

export default function InstanceAdminPage() {
  const navigate = useNavigate();
  const { data: currentUser } = useCurrentUser();
  const { instances, activeInstanceUrl } = useInstanceStore();
  const [tab, setTab] = useState<AdminTab>('users');
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [auditLoading, setAuditLoading] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionLoading, setActionLoading] = useState<string | null>(null);

  const instance = instances.find((i) => i.url === activeInstanceUrl);

  const instanceLabel = (url: string) => {
    try {
      return new URL(url).hostname;
    } catch {
      return url;
    }
  };

  const fetchUsers = useCallback(async () => {
    if (!activeInstanceUrl) return;
    setLoading(true);
    setError(null);
    try {
      const apiClient = instanceManager.get(activeInstanceUrl).api;
      const res = await apiClient.typed((c) => c.GET('/admin/users', { params: { query: { limit: 200 } } }));
      setUsers(res.data);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to load users');
    } finally {
      setLoading(false);
    }
  }, [activeInstanceUrl]);

  const fetchAudit = useCallback(async () => {
    if (!activeInstanceUrl) return;
    setAuditLoading(true);
    setError(null);
    try {
      const apiClient = instanceManager.get(activeInstanceUrl).api;
      const res = await apiClient.typed((c) =>
        c.GET('/admin/audit-log', { params: { query: { limit: 200 } } }),
      );
      setEntries(res.data);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to load the audit log');
    } finally {
      setAuditLoading(false);
    }
  }, [activeInstanceUrl]);

  const refresh = useCallback(() => {
    if (tab === 'audit') {
      fetchAudit();
      return;
    }
    fetchUsers();
  }, [fetchAudit, fetchUsers, tab]);

  useEffect(() => {
    if (!currentUser?.is_instance_admin) {
      navigate('/app', { replace: true });
      return;
    }
    if (tab === 'audit') {
      fetchAudit();
      return;
    }
    fetchUsers();
  }, [activeInstanceUrl, currentUser, fetchAudit, fetchUsers, navigate, tab]);

  const handleRoleChange = async (user: AdminUser, isAdmin: boolean) => {
    if (!activeInstanceUrl) return;
    if (user.id === currentUser?.id) return;
    setActionLoading(user.id + '_admin');
    try {
      const apiClient = instanceManager.get(activeInstanceUrl).api;
      await apiClient.typed((c) =>
        c.PATCH('/admin/users/{user_id}/instance-role', {
          params: { path: { user_id: user.id } },
          body: { is_instance_admin: isAdmin },
        }),
      );
      setUsers((prev) => prev.map((u) => (u.id === user.id ? { ...u, is_instance_admin: isAdmin } : u)));
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Action failed');
    } finally {
      setActionLoading(null);
    }
  };

  const handleToggleStatus = async (user: AdminUser) => {
    if (!activeInstanceUrl) return;
    if (user.id === currentUser?.id) return;
    const action = user.status === 'active' ? 'suspend' : 'activate';
    setActionLoading(user.id + '_status');
    try {
      const apiClient = instanceManager.get(activeInstanceUrl).api;
      const params = { path: { user_id: user.id } };
      if (action === 'suspend') {
        await apiClient.typed((c) => c.POST('/admin/users/{user_id}/suspend', { params }));
      } else {
        await apiClient.typed((c) => c.POST('/admin/users/{user_id}/activate', { params }));
      }
      setUsers((prev) =>
        prev.map((u) =>
          u.id === user.id ? { ...u, status: action === 'suspend' ? 'suspended' : 'active' } : u,
        ),
      );
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Action failed');
    } finally {
      setActionLoading(null);
    }
  };

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
          disabled={loading || auditLoading}
          className="flex items-center gap-2 px-3 py-2 text-sm text-muted hover:text-fg hover:bg-surface rounded-lg transition cursor-pointer disabled:opacity-50 shrink-0"
        >
          <RefreshCw className={`w-4 h-4 ${loading || auditLoading ? 'animate-spin' : ''}`} />
          <span className="max-sm:sr-only">Refresh</span>
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-4 sm:p-6">
        {error && (
          <div className="bg-red-500/10 border border-red-500/30 text-danger px-4 py-3 rounded-lg mb-4 text-sm">
            {error}
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
                          {actionLoading === user.id + '_admin' ? (
                            <div className="w-4 h-4 border border-purple-400/30 border-t-purple-400 rounded-full animate-spin" />
                          ) : (
                            <select
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
                              disabled={
                                isSelf || actionLoading === user.id + '_status' || user.status === 'pending'
                              }
                              className="flex items-center gap-1 px-2 py-1.5 rounded text-xs transition cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed text-muted hover:text-danger hover:bg-raised"
                              title={user.status === 'active' ? 'Suspend user' : 'Activate user'}
                            >
                              {actionLoading === user.id + '_status' ? (
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
