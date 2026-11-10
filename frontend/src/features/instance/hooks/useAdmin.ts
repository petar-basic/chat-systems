import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type AdminUser = components['schemas']['AdminUser'];
export type EmailTest = components['schemas']['EmailTest'];
export type InstanceSettings = components['schemas']['InstanceSettings'];
export type InstanceRole = components['schemas']['InstanceRole'];
export type AdminWorkspace = components['schemas']['AdminWorkspace'];
export type WorkspacePerson = components['schemas']['WorkspacePerson'];

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
    mutationFn: async ({ userId, role }: { userId: string; role: InstanceRole }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PATCH('/admin/users/{user_id}/instance-role', {
          params: { path: { user_id: userId } },
          body: { role },
        }),
      ),
    onSuccess: ({ role }, { userId }) => {
      queryClient.setQueryData<AdminUser[]>(key, (old) => patchUser(old, userId, { instance_role: role }));
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

export const useSendTestEmail = (instanceUrl: string | null) =>
  useMutation({
    mutationFn: () => getApiForInstance(instanceUrl).typed((c) => c.POST('/admin/email/test')),
  });

export const useInstanceSettings = (instanceUrl: string | null) =>
  useQuery({
    queryKey: QUERY_KEYS.adminSettings(instanceUrl ?? ''),
    queryFn: () => getApiForInstance(instanceUrl).typed((c) => c.GET('/admin/settings')),
    enabled: !!instanceUrl,
  });

export const useUpdateInstanceSettings = (instanceUrl: string | null) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: components['schemas']['UpdateInstanceSettings']) =>
      getApiForInstance(instanceUrl).typed((c) => c.PATCH('/admin/settings', { body })),
    onSuccess: (settings) => {
      queryClient.setQueryData<InstanceSettings>(QUERY_KEYS.adminSettings(instanceUrl ?? ''), settings);
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.instanceInfo(instanceUrl ?? '') });
    },
  });
};

export const useAdminWorkspaces = (instanceUrl: string | null) =>
  useQuery({
    queryKey: QUERY_KEYS.adminWorkspaces(instanceUrl ?? ''),
    queryFn: async () => {
      if (!instanceUrl) throw new Error('No instance');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/admin/workspaces', { params: { query: { limit: PAGE } } }),
      );
      return res.data;
    },
    enabled: !!instanceUrl,
    staleTime: 1000 * 30,
  });

export const useAdminWorkspacePeople = (instanceUrl: string | null, workspaceId: string) =>
  useQuery({
    queryKey: QUERY_KEYS.adminWorkspacePeople(instanceUrl ?? '', workspaceId),
    queryFn: async () => {
      if (!instanceUrl) throw new Error('No instance');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/admin/workspaces/{ws_id}/members', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.data;
    },
    enabled: !!instanceUrl,
  });

export const useAppointOwner = (instanceUrl: string | null) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ workspaceId, userId }: { workspaceId: string; userId: string }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/admin/workspaces/{ws_id}/owners', {
          params: { path: { ws_id: workspaceId } },
          body: { user_id: userId },
        }),
      ),
    onSuccess: (_, { workspaceId }) => {
      void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.adminWorkspaces(instanceUrl ?? '') });
      void queryClient.invalidateQueries({
        queryKey: QUERY_KEYS.adminWorkspacePeople(instanceUrl ?? '', workspaceId),
      });
    },
  });
};
