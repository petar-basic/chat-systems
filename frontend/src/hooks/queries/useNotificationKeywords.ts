import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export const useNotificationKeywords = (workspaceId: string | null, instanceUrl?: string) =>
  useQuery({
    queryKey: QUERY_KEYS.notificationKeywords(workspaceId ?? ''),
    queryFn: async () => {
      if (!workspaceId) throw new Error('No workspace ID');
      const res = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/notification-keywords', { params: { path: { ws_id: workspaceId } } }),
      );
      return res.keywords;
    },
    enabled: !!workspaceId && !!instanceUrl,
    staleTime: 1000 * 60 * 5,
  });

export const useSetNotificationKeywords = (workspaceId: string, instanceUrl?: string) => {
  const queryClient = useQueryClient();
  const key = QUERY_KEYS.notificationKeywords(workspaceId);
  return useMutation({
    mutationFn: (keywords: string[]) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.PUT('/workspaces/{ws_id}/notification-keywords', {
          params: { path: { ws_id: workspaceId } },
          body: { keywords },
        }),
      ),
    onMutate: (keywords) => {
      const previous = queryClient.getQueryData<string[]>(key);
      queryClient.setQueryData(key, keywords);
      return { previous };
    },
    onError: (_err, _keywords, context) => {
      queryClient.setQueryData(key, context?.previous);
    },
    onSuccess: (saved) => {
      queryClient.setQueryData(key, saved.keywords);
    },
  });
};
