import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type AdminUser = components['schemas']['AdminUser'];

const PAGE = 200;

export const useAdminUsers = (instanceUrl: string | null) =>
  useQuery({
    queryKey: QUERY_KEYS.adminUsers(instanceUrl ?? ''),
    queryFn: async () => {
      if (!instanceUrl) throw new Error('No instance');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/admin/users', { params: { query: { limit: PAGE } } }),
      );
      return res.data;
    },
    enabled: !!instanceUrl,
    staleTime: 1000 * 30,
  });

export const useAdminAuditLog = (instanceUrl: string | null) =>
  useQuery({
    queryKey: QUERY_KEYS.adminAuditLog(instanceUrl ?? ''),
    queryFn: async () => {
      if (!instanceUrl) throw new Error('No instance');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/admin/audit-log', { params: { query: { limit: PAGE } } }),
      );
      return res.data;
    },
    enabled: !!instanceUrl,
    staleTime: 1000 * 15,
  });

function patchUser(users: AdminUser[] | undefined, userId: string, patch: Partial<AdminUser>) {
  return users?.map((u) => (u.id === userId ? { ...u, ...patch } : u));
}

export const useSetInstanceRole = (instanceUrl: string | null) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.adminUsers(instanceUrl ?? '');
  return useMutation({
    mutationFn: async ({ userId, isAdmin }: { userId: string; isAdmin: boolean }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PATCH('/admin/users/{user_id}/instance-role', {
          params: { path: { user_id: userId } },
          body: { is_instance_admin: isAdmin },
        }),
      ),
    onSuccess: (_, { userId, isAdmin }) => {
      queryClient.setQueryData<AdminUser[]>(key, (old) =>
        patchUser(old, userId, { is_instance_admin: isAdmin }),
      );
    },
  });
};

export const useSetUserStatus = (instanceUrl: string | null) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.adminUsers(instanceUrl ?? '');
  return useMutation({
    mutationFn: async ({ userId, suspend }: { userId: string; suspend: boolean }) => {
      const params = { path: { user_id: userId } };
      const api = getApiForInstance(instanceUrl);
      if (suspend) await api.typed((c) => c.POST('/admin/users/{user_id}/suspend', { params }));
      else await api.typed((c) => c.POST('/admin/users/{user_id}/activate', { params }));
    },
    onSuccess: (_, { userId, suspend }) => {
      queryClient.setQueryData<AdminUser[]>(key, (old) =>
        patchUser(old, userId, { status: suspend ? 'suspended' : 'active' }),
      );
    },
  });
};
