import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export const useAccount = (instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.account(instanceUrl ?? ''),
    queryFn: async () => getApiForInstance(instanceUrl).typed((c) => c.GET('/users/me')),
    enabled: !!instanceUrl,
    staleTime: 1000 * 60,
  });
};

export const useWorkspaceProfile = (workspaceId?: string, instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.workspaceProfile(workspaceId ?? ''),
    queryFn: async () =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/profile', { params: { path: { ws_id: workspaceId ?? '' } } }),
      ),
    enabled: !!workspaceId && !!instanceUrl,
  });
};

export const useSaveWorkspaceProfile = (workspaceId?: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  const path = { params: { path: { ws_id: workspaceId ?? '' } } };
  const refresh = async () => {
    if (!workspaceId) return;
    await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceProfile(workspaceId) });
    await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });
  };

  const profile = useMutation({
    mutationFn: async (body: { display_name: string; bio: string; avatar_url: string }) =>
      getApiForInstance(instanceUrl).typed((c) => c.PATCH('/workspaces/{ws_id}/profile', { ...path, body })),
  });

  const setStatus = useMutation({
    mutationFn: async ({
      emoji,
      text,
      expiresAt,
    }: {
      emoji: string | null;
      text: string | null;
      expiresAt: Date | null;
    }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PUT('/workspaces/{ws_id}/profile/status', {
          ...path,
          body: { emoji, text, expires_at: expiresAt ? expiresAt.toISOString() : null },
        }),
      ),
  });

  const clearStatus = useMutation({
    mutationFn: async () =>
      getApiForInstance(instanceUrl).typed((c) => c.DELETE('/workspaces/{ws_id}/profile/status', path)),
  });

  return { profile, setStatus, clearStatus, refresh };
};
