import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { useWorkspaceStore } from '@/stores/workspace';

type EmailPreference = components['schemas']['EmailPreference'];

export const useEmailPreference = () => {
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const workspaceId = workspace?.id ?? '';
  return useQuery({
    queryKey: QUERY_KEYS.notificationEmail(workspaceId),
    queryFn: () =>
      getApiForInstance(workspace?.instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/notifications/email', { params: { path: { ws_id: workspaceId } } }),
      ),
    enabled: !!workspace,
  });
};

export const useSetEmailPreference = () => {
  const queryClient = useQueryClient();
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const workspaceId = workspace?.id ?? '';
  return useMutation({
    mutationFn: (mentionEmails: boolean) =>
      getApiForInstance(workspace?.instanceUrl).typed((c) =>
        c.PATCH('/workspaces/{ws_id}/notifications/email', {
          params: { path: { ws_id: workspaceId } },
          body: { mention_emails: mentionEmails },
        }),
      ),
    onSuccess: (saved: EmailPreference) => {
      queryClient.setQueryData(QUERY_KEYS.notificationEmail(workspaceId), saved);
    },
  });
};
