import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export const useMyStatus = (instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.myStatus(instanceUrl ?? ''),
    queryFn: async () => getApiForInstance(instanceUrl).typed((c) => c.GET('/users/me')),
    enabled: !!instanceUrl,
    staleTime: 1000 * 60,
  });
};

export const useSetStatus = (instanceUrl?: string, workspaceId?: string) => {
  const queryClient = useQueryClient();
  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.myStatus(instanceUrl ?? '') });
    if (workspaceId) {
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspaceId) });
    }
  };

  const set = useMutation({
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
        c.PUT('/users/me/status', {
          body: { emoji, text, expires_at: expiresAt ? expiresAt.toISOString() : null },
        }),
      ),
    onSuccess: refresh,
  });

  const clear = useMutation({
    mutationFn: async () => getApiForInstance(instanceUrl).typed((c) => c.DELETE('/users/me/status')),
    onSuccess: refresh,
  });

  return { set, clear };
};
