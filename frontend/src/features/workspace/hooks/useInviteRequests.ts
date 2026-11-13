import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { components } from '@/api/schema';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';

export type InviteRequest = components['schemas']['WorkspaceInviteRequest'];
export type RequestTerms = components['schemas']['RequestTerms'];
export type AskForInvites = components['schemas']['AskForInvites'];

export const useInviteRequests = (workspaceId: string, instanceUrl?: string, enabled = true) =>
  useQuery({
    queryKey: QUERY_KEYS.inviteRequests(workspaceId),
    queryFn: async () => {
      const response = await getApiForInstance(instanceUrl).typed((c) =>
        c.GET('/workspaces/{ws_id}/invite-requests', { params: { path: { ws_id: workspaceId } } }),
      );
      return response.data;
    },
    enabled: !!workspaceId && enabled,
    staleTime: 1000 * 30,
  });

function useRefresh(workspaceId: string) {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.inviteRequests(workspaceId) });
    void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceInvites(workspaceId) });
  };
}

export const useAskForInvites = (workspaceId: string, instanceUrl?: string) => {
  const refresh = useRefresh(workspaceId);
  return useMutation({
    mutationFn: (body: AskForInvites) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/invite-requests', { params: { path: { ws_id: workspaceId } }, body }),
      ),
    onSuccess: refresh,
  });
};

export const useApproveInviteRequest = (workspaceId: string, instanceUrl?: string) => {
  const refresh = useRefresh(workspaceId);
  return useMutation({
    mutationFn: ({ requestId, terms }: { requestId: string; terms?: RequestTerms }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/invite-requests/{request_id}/approve', {
          params: { path: { ws_id: workspaceId, request_id: requestId } },
          body: { terms: terms ?? null },
        }),
      ),
    onSuccess: refresh,
  });
};

export const useDeclineInviteRequest = (workspaceId: string, instanceUrl?: string) => {
  const refresh = useRefresh(workspaceId);
  return useMutation({
    mutationFn: ({ requestId, note }: { requestId: string; note: string }) =>
      getApiForInstance(instanceUrl).typed((c) =>
        c.POST('/workspaces/{ws_id}/invite-requests/{request_id}/decline', {
          params: { path: { ws_id: workspaceId, request_id: requestId } },
          body: { note: note.trim() || null },
        }),
      ),
    onSuccess: refresh,
  });
};
