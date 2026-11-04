import { useMutation, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore } from '@/stores/workspace';

export const useUpdateTimezone = () => {
  const queryClient = useQueryClient();
  const instanceUrl = useWorkspaceStore((s) => s.currentWorkspace?.instanceUrl);
  return useMutation({
    mutationFn: (timezone: string) =>
      getApiForInstance(instanceUrl).typed((c) => c.PATCH('/users/me', { body: { timezone } })),
    onSuccess: async (account) => {
      queryClient.setQueryData(QUERY_KEYS.account(instanceUrl ?? ''), account);
      await queryClient.invalidateQueries({
        predicate: (q) =>
          (q.queryKey[0] === 'notifications' && q.queryKey[1] === 'schedule') ||
          (q.queryKey[0] === 'workspaces' && q.queryKey[2] === 'members'),
      });
    },
  });
};
