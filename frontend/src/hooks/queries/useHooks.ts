import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type HookType = 'incoming_webhook' | 'outgoing_webhook' | 'bot' | 'slash_command' | 'scheduled';

export interface Hook {
  id: string;
  workspace_id: string;
  created_by: string;
  hook_type: HookType;
  name: string;
  description: string | null;
  config: Record<string, unknown>;
  is_active: boolean;
  created_at: string;
}

export interface HookSecrets {
  hook_id: string;
  hook_type: HookType;
  config: Record<string, unknown>;
  incoming_url: string | null;
}

export const useHooks = (workspaceId: string | null, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.hooks(workspaceId ?? ''),
    queryFn: async () => {
      const res = await getApiForInstance(instanceUrl).get<{ data: Hook[] }>(
        `/workspaces/${workspaceId}/hooks`,
      );
      return res.data;
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 30,
  });
};

export const useCreateHook = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: { hook_type: HookType; name: string; config: Record<string, unknown> }) =>
      getApiForInstance(instanceUrl).post<Hook>(`/workspaces/${workspaceId}/hooks`, body),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.hooks(workspaceId) });
    },
  });
};

export const useDeleteHook = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (hookId: string) => getApiForInstance(instanceUrl).delete(`/hooks/${hookId}`),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.hooks(workspaceId) });
    },
  });
};

export const useRevealHook = (instanceUrl?: string) => {
  return useMutation({
    mutationFn: async (hookId: string) =>
      getApiForInstance(instanceUrl).post<HookSecrets>(`/hooks/${hookId}/reveal`, {}),
  });
};

export const useRotateHook = (instanceUrl?: string) => {
  return useMutation({
    mutationFn: async (hookId: string) =>
      getApiForInstance(instanceUrl).post<HookSecrets>(`/hooks/${hookId}/rotate`, {}),
  });
};
