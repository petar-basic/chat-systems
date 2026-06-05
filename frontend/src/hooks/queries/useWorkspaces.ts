import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useInstanceStore } from '@/stores/instances';
import { useWorkspaceStore } from '@/stores/workspace';
import { instanceManager } from '@/lib/instances';
import { api } from '@/lib/api';
import { QUERY_KEYS } from '@/shared/constants';
import type { Workspace, WorkspaceMember, Channel } from '@/stores/workspace';

interface WorkspacesResponse {
  data: Omit<Workspace, 'instanceUrl'>[];
}

interface WorkspaceMembersResponse {
  data: WorkspaceMember[];
}

interface ChannelsResponse {
  data: Channel[];
}

export const useWorkspaces = () => {
  const instances = useInstanceStore((s) => s.instances);
  const instanceUrls = instances
    .map((i) => i.url)
    .sort()
    .join(',');

  return useQuery({
    queryKey: QUERY_KEYS.workspacesList(instanceUrls),
    queryFn: async (): Promise<Workspace[]> => {
      const results = await Promise.allSettled(
        instances.map(async (inst) => {
          const clients = instanceManager.get(inst.url);
          const res = await clients.api.get<WorkspacesResponse>('/workspaces');
          return res.data.map((ws) => ({ ...ws, instanceUrl: inst.url }));
        }),
      );
      return results
        .filter((r): r is PromiseFulfilledResult<Workspace[]> => r.status === 'fulfilled')
        .flatMap((r) => r.value);
    },
    enabled: instances.length > 0,
    staleTime: 1000 * 60 * 5,
  });
};

export const useWorkspace = (workspaceId: string | null) => {
  const queryClient = useQueryClient();

  return useQuery({
    queryKey: QUERY_KEYS.workspace(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace ID');
      const cached = queryClient.getQueryData<Workspace[]>(
        QUERY_KEYS.workspacesList(
          useInstanceStore
            .getState()
            .instances.map((i) => i.url)
            .sort()
            .join(','),
        ),
      );
      const instanceUrl = cached?.find((w) => w.id === workspaceId)?.instanceUrl;
      const apiClient = instanceUrl ? instanceManager.get(instanceUrl).api : undefined;
      if (!apiClient) throw new Error('Instance not found for workspace');
      const response = await apiClient.get<Workspace>(`/workspaces/${workspaceId}`);
      return { ...response, instanceUrl };
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 60 * 5,
  });
};

export const useWorkspaceMembers = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.workspaceMembers(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace ID');
      const apiClient = instanceUrl ? instanceManager.get(instanceUrl).api : api;
      const response = await apiClient.get<WorkspaceMembersResponse>(`/workspaces/${workspaceId}/members`);
      return response.data;
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 60 * 2,
  });
};

export const useWorkspaceChannels = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.workspaceChannels(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace ID');
      const apiClient = instanceUrl ? instanceManager.get(instanceUrl).api : api;
      const response = await apiClient.get<ChannelsResponse>(`/workspaces/${workspaceId}/channels`);
      return response.data;
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: 1000 * 60 * 2,
  });
};

export const useDeletedWorkspaces = () => {
  const instances = useInstanceStore((s) => s.instances);
  const instanceUrls = instances
    .map((i) => i.url)
    .sort()
    .join(',');

  return useQuery({
    queryKey: QUERY_KEYS.deletedWorkspacesList(instanceUrls),
    queryFn: async (): Promise<Workspace[]> => {
      const results = await Promise.allSettled(
        instances.map(async (inst) => {
          const clients = instanceManager.get(inst.url);
          const res = await clients.api.get<WorkspacesResponse>('/workspaces/deleted');
          return res.data.map((ws) => ({ ...ws, instanceUrl: inst.url }));
        }),
      );
      return results
        .filter((r): r is PromiseFulfilledResult<Workspace[]> => r.status === 'fulfilled')
        .flatMap((r) => r.value);
    },
    enabled: instances.length > 0,
    staleTime: 1000 * 60 * 5,
  });
};

export const useRestoreWorkspace = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ workspaceId, instanceUrl }: { workspaceId: string; instanceUrl: string }) => {
      const clients = instanceManager.get(instanceUrl);
      return clients.api.post<Workspace>(`/workspaces/${workspaceId}/restore`, {});
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
    },
  });
};

export const useCreateWorkspace = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ name, instanceUrl }: { name: string; instanceUrl: string }) => {
      const clients = instanceManager.get(instanceUrl);
      const ws = await clients.api.post<Omit<Workspace, 'instanceUrl'>>('/workspaces', { name });
      return { ...ws, instanceUrl } as Workspace;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
    },
  });
};

export const useCreateChannel = () => {
  const queryClient = useQueryClient();
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);

  return useMutation({
    mutationFn: async ({
      workspaceId,
      name,
      type = 'public',
    }: {
      workspaceId: string;
      name: string;
      type?: string;
    }) => {
      const apiClient = currentWorkspace?.instanceUrl
        ? instanceManager.get(currentWorkspace.instanceUrl).api
        : api;
      return apiClient.post<Channel>(`/workspaces/${workspaceId}/channels`, { name, channel_type: type });
    },
    onSuccess: (_, { workspaceId }) => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
    },
  });
};
