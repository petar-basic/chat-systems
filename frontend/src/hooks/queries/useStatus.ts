import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export interface UserStatus {
  status_emoji: string | null;
  status_text: string | null;
  status_expires_at: string | null;
}

export const useMyStatus = (instanceUrl?: string) => {
  return useQuery({
    queryKey: QUERY_KEYS.myStatus(instanceUrl ?? ''),
    queryFn: async () => getApiForInstance(instanceUrl).get<UserStatus>('/users/me'),
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
      getApiForInstance(instanceUrl).put<UserStatus>('/users/me/status', {
        emoji,
        text,
        expires_at: expiresAt ? expiresAt.toISOString() : null,
      }),
    onSuccess: refresh,
  });

  const clear = useMutation({
    mutationFn: async () => getApiForInstance(instanceUrl).delete<UserStatus>('/users/me/status'),
    onSuccess: refresh,
  });

  return { set, clear };
};
