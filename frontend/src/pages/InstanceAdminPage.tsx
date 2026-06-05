import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { UserX, UserCheck, ArrowLeft, RefreshCw } from 'lucide-react';
import { useInstanceStore } from '../stores/instances';
import { instanceManager } from '../lib/instances';
import { useCurrentUser } from '../hooks/queries/useAuth';

interface AdminUser {
  id: string;
  email: string;
  display_name: string | null;
  status: string;
  is_instance_admin: boolean;
  created_at: string;
}

export default function InstanceAdminPage() {
  const navigate = useNavigate();
  const { data: currentUser } = useCurrentUser();
  const { instances, activeInstanceUrl } = useInstanceStore();
  const [users, setUsers] = useState<AdminUser[]>([]);
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
      const res = await apiClient.get<{ data: AdminUser[] }>('/admin/users?limit=200');
      setUsers(res.data);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to load users');
    } finally {
      setLoading(false);
    }
  }, [activeInstanceUrl]);

  useEffect(() => {
    if (!currentUser?.is_instance_admin) {
      navigate('/app', { replace: true });
      return;
    }
    fetchUsers();
  }, [activeInstanceUrl, currentUser, fetchUsers, navigate]);

  const handleRoleChange = async (user: AdminUser, isAdmin: boolean) => {
    if (!activeInstanceUrl) return;
    if (user.id === currentUser?.id) return;
    setActionLoading(user.id + '_admin');
    try {
      const apiClient = instanceManager.get(activeInstanceUrl).api;
      await apiClient.patch(`/admin/users/${user.id}/instance-role`, {
        is_instance_admin: isAdmin,
      });
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
      await apiClient.post(`/admin/users/${user.id}/${action}`, {});
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
    <div className="h-screen bg-slate-900 text-white flex flex-col">
      <div className="border-b border-slate-800 px-6 py-4 flex items-center gap-4">
        <button
          onClick={() => navigate('/app')}
          className="p-2 text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition cursor-pointer"
        >
          <ArrowLeft className="w-5 h-5" />
        </button>
        <div className="flex-1">
          <h1 className="text-lg font-bold">Instance Admin</h1>
          {instance && <p className="text-xs text-slate-400">{instanceLabel(instance.url)}</p>}
        </div>
        <button
          onClick={fetchUsers}
          disabled={loading}
          className="flex items-center gap-2 px-3 py-2 text-sm text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition cursor-pointer disabled:opacity-50"
        >
          <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
          Refresh
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-6">
        {error && (
          <div className="bg-red-500/10 border border-red-500/30 text-red-400 px-4 py-3 rounded-lg mb-4 text-sm">
            {error}
          </div>
        )}

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <div className="w-8 h-8 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : (
          <div className="max-w-4xl mx-auto">
            <div className="mb-4 text-sm text-slate-400">{users.length} users on this instance</div>

            <div className="bg-slate-800 rounded-xl border border-slate-700 overflow-hidden">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-slate-700">
                    <th className="text-left px-4 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">
                      User
                    </th>
                    <th className="text-left px-4 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">
                      Status
                    </th>
                    <th className="text-left px-4 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">
                      Role
                    </th>
                    <th className="text-right px-4 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">
                      Actions
                    </th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-700">
                  {users.map((user) => {
                    const isSelf = user.id === currentUser?.id;
                    return (
                      <tr key={user.id} className="hover:bg-slate-700/30 transition">
                        <td className="px-4 py-3">
                          <div className="font-medium text-sm text-white">
                            {user.display_name || '(no name)'}
                            {isSelf && <span className="ml-2 text-xs text-purple-400">(you)</span>}
                          </div>
                          <div className="text-xs text-slate-400">{user.email}</div>
                        </td>
                        <td className="px-4 py-3">
                          <span
                            className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                              user.status === 'active'
                                ? 'bg-green-500/10 text-green-400'
                                : user.status === 'pending'
                                  ? 'bg-yellow-500/10 text-yellow-400'
                                  : 'bg-red-500/10 text-red-400'
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
                              className="text-xs bg-slate-700 border border-slate-600 rounded px-2 py-1 text-white focus:outline-none focus:ring-1 focus:ring-purple-500 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
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
                              className="flex items-center gap-1 px-2 py-1.5 rounded text-xs transition cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed text-slate-400 hover:text-red-400 hover:bg-slate-700"
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
