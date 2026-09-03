import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { useInstanceStore } from '@/stores/instances';
import { useWorkspaceStore } from '@/stores/workspace';
import { instanceManager } from '@/lib/instances';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import type { Workspace, Channel } from '@/stores/workspace';

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
          const res = await getApiForInstance(inst.url).typed((c) => c.GET('/workspaces'));
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
      const apiClient = instanceUrl ? getApiForInstance(instanceUrl) : undefined;
      if (!apiClient) throw new Error('Instance not found for workspace');
      const response = await apiClient.typed((c) =>
        c.GET('/workspaces/{ws_id}', { params: { path: { ws_id: workspaceId } } }),
      );
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
      const response = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/members', { params: { path: { ws_id: workspaceId } } }),
      );
      return response.data;
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 60 * 2,
  });
};

export const useWorkspaceChannels = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.workspaceChannels(workspaceId ?? ''),
    queryFn: async (): Promise<Channel[]> => {
      if (!workspaceId) throw new Error('No workspace ID');
      const response = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/channels', { params: { path: { ws_id: workspaceId } } }),
      );
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
          const res = await getApiForInstance(inst.url).typed((c) => c.GET('/workspaces/deleted'));
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
    mutationFn: async ({ workspaceId, instanceUrl }: { workspaceId: string; instanceUrl: string }) =>
      instanceManager
        .get(instanceUrl)
        .api.typed((c) =>
          c.POST('/workspaces/{ws_id}/restore', { params: { path: { ws_id: workspaceId } } }),
        ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
    },
  });
};

export const useCreateWorkspace = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ name, instanceUrl }: { name: string; instanceUrl: string }): Promise<Workspace> => {
      const ws = await instanceManager
        .get(instanceUrl)
        .api.typed((c) => c.POST('/workspaces', { body: { name } }));
      return { ...ws, instanceUrl };
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
      description,
      postPolicy,
    }: {
      workspaceId: string;
      name: string;
      type?: components['schemas']['ChannelType'];
      description?: string;
      postPolicy?: components['schemas']['PostPolicy'];
    }) =>
      getApiForInstance(currentWorkspace?.instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/channels', {
          params: { path: { ws_id: workspaceId } },
          body: {
            name,
            channel_type: type,
            description: description || null,
            post_policy: postPolicy ?? null,
          },
        }),
      ),
    onSuccess: (_, { workspaceId }) => {
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceChannels(workspaceId) });
    },
  });
};
