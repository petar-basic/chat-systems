import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type Hook = components['schemas']['Hook'];

export type HookSecrets = components['schemas']['HookSecrets'];

export const useHooks = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.hooks(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/hooks', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.data;
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 30,
  });
};

/**
 * Which channels forward their traffic off the instance. Readable by every member,
 * not just admins: the point is that people can see it before they type.
 */
export const useHookedChannels = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.hookedChannels(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace selected');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/hooks/channels', { params: { path: { ws_id: workspaceId } } }),
      );
      return new Set(res.channel_ids);
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 60,
  });
};

export const useCreateHook = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: components['schemas']['CreateHookRequest']) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/hooks', { params: { path: { ws_id: workspaceId } }, body }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.hooks(workspaceId) });
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.hookedChannels(workspaceId) });
    },
  });
};

export const useDeleteHook = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (hookId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.DELETE('/hooks/{hook_id}', { params: { path: { hook_id: hookId } } }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.hooks(workspaceId) });
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.hookedChannels(workspaceId) });
    },
  });
};

export const useRevealHook = (instanceUrl?: string) => {
  return useMutation({
    mutationFn: async (hookId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/hooks/{hook_id}/reveal', { params: { path: { hook_id: hookId } } }),
      ),
  });
};

export const useRotateHook = (instanceUrl?: string) => {
  return useMutation({
    mutationFn: async (hookId: string) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/hooks/{hook_id}/rotate', { params: { path: { hook_id: hookId } } }),
      ),
  });
};
